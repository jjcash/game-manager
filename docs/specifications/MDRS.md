# Game Manager

Revision: reconciled 2026-10-25 against the original [Game Manager Annotations Review](https://chatgpt.com/c/6add59c2-80bc-83ea-a64e-e890c3f606f6) and later user decisions. See [source and decision register](../SOURCE_AND_OPEN_DECISIONS.md).

**Implementation status: not started.** Specifications, interface sketches, SQL examples, and acceptance targets describe intended behavior; they are not existing code or verified results. Later explicit decisions override earlier drafts: installed Windows only, no portable distribution, no Playnite migration requirement, and external trainers first in later Game Tools work.

## Mod Deployment & Recovery Specification — Reconciled v0.2

**Status:** Proposed for acceptance  
**Implementation status:** Not started  
**Primary platform:** Windows (installed version only)  
**Implementation language:** Rust  
**Persistence:** SQLite + durable filesystem journal  
**Architecture:** Hybrid deployment, game-specific adapters, local-first

---

## 1. Purpose

This specification defines how Game Manager will install, activate, deactivate, update, and recover mods across different games and modding ecosystems.

It is designed to support a wide variety of games without assuming they all use the same installation mechanism.

The initial implementation will prioritize reliability over maximum compatibility.

The mod-management subsystem must:

- Support different modding frameworks through adapters.
- Preserve original game files.
- Track all Game Manager-owned filesystem changes.
- Support multiple mod profiles.
- Detect existing mods without taking ownership automatically.
- Preview conflicts and intended filesystem operations.
- Recover safely after application crashes or interrupted operations.
- Perform expensive operations without blocking the application interface.
- Remain independent of optional cloud services.
- Allow future deployment strategies without changing the profile model.

## 2. Non-goals

The initial implementation will not attempt to:

- Manage every moddable game automatically.
- Replace advanced external mod managers immediately.
- Implement a universal virtual filesystem.
- Automatically execute mod installers or scripts.
- Guarantee compatibility between arbitrary mods.
- Repair game installations modified by unrelated tools.
- Bypass DRM, anti-cheat systems, or platform security.
- Install or modify game files silently.
- Provide native trainers as part of mod deployment.
- Support portable installation or relocation workflows.

## 3. Core terminology

**Mod:** A logical modification, identified independently of any installed archive.

**Mod Version:** One release or variant of a mod.

**Mod Package:** An immutable local archive or managed package containing mod files.

**Mod Installation Record:** The association between a mod and a particular game installation.

**Mod Profile:** A named selection of mods, versions, ordering rules, and installation options.

**Game Adapter:** The game-specific component that interprets modding conventions.

**Activation Strategy:** The method by which a mod becomes active in a game.

**Deployment Backend:** The implementation that performs and reverses filesystem changes.

**Deployment Plan:** A validated description of intended changes before execution.

**Deployment Manifest:** A persistent description of changes actually performed.

**Original Baseline:** The preserved state of files before Game Manager first modified them.

**Recovery Journal:** A durable record of filesystem operations and their completion states.

## 4. Architectural components

The system consists of eight cooperating components.

| Component | Responsibility |
| --- | --- |
| Mod Inventory | Identify and catalog mods, versions, and packages |
| Package Inspector | Safely inspect archives and file structures |
| Game Adapter Registry | Select game-specific compatibility logic |
| Profile Manager | Maintain mod selections, versions, and priorities |
| Deployment Planner | Compute intended filesystem or launch-time changes |
| Conflict Analyzer | Identify overlapping files and known incompatibilities |
| Deployment Executor | Apply and reverse validated operations |
| Recovery Coordinator | Reconcile unfinished operations and restore valid states |

The planned Rust background-job scheduler will coordinate expensive operations; no scheduler exists yet.

The launcher consumes the resulting activation or deployment state; it does not implement mod installation itself.

## 5. Separation of responsibilities

### 5.1 Game adapters

Game adapters determine:

- Whether an installation supports modding.
- Supported mod formats and loaders.
- Valid target directories.
- Whether load order matters.
- Required game or loader versions.
- Whether mod activation requires filesystem changes.
- Required launch arguments and environment variables.
- Whether profile switching is supported.
- Additional game-specific validation rules.

Adapters produce a structured activation plan.

They must not directly modify game installation files.

### 5.2 Deployment backends

Deployment backends determine how authorized filesystem changes are executed.

They must not decide whether a mod is compatible with a particular game.

Supported architectural strategies:

| Strategy | Initial implementation | Notes |
| --- | --- | --- |
| Managed copy | Yes | Initial general-purpose filesystem backend |
| Launch-time activation | Yes, when supported by an adapter | May require no file deployment |
| Symbolic links | Later | Filesystem and permissions dependent |
| Hard links | Later | Same-volume restrictions; shared content risks |
| Virtual filesystem | Long-term | Requires separate compatibility and security design |

Some game adapters may combine a deployment backend with launch-time activation rules.

## 6. Mod package lifecycle

A mod package progresses through the following states:

1. **Discovered:** A local archive or existing mod has been identified.
2. **Inspected:** File structure and metadata have been examined.
3. **Imported:** The package is preserved in managed storage.
4. **Available:** The package can be added to compatible profiles.
5. **Referenced:** One or more profiles or deployments depend on it.
6. **Unreferenced:** The package is no longer required by any managed configuration.
7. **Eligible for cleanup:** A separate safe garbage-collection operation may remove it.

Mod archives must never be modified in place.

Every imported package receives a content hash and a stable package identifier.

The package inspector may generate a normalized file manifest and determine whether additional game-specific inspection is necessary.

Unknown or unsupported packages can remain cataloged without being deployable.

## 7. Managed storage model

The application will maintain a managed mod repository outside game installations.

Proposed logical layout:

```
GameManagerData/
  mods/
    packages/
      <content-hash>
    extracted/
      <package-id>/
    staging/
      <operation-id>/
    manifests/
  deployments/
    journals/
      <operation-id>/
    originals/
      <installation-id>/
    recovery/
  database/
    game-manager.db
  logs/
```

This layout represents logical ownership, not necessarily one physical drive.

### Storage requirements

- Package objects are immutable.
- Temporary files are never considered installed merely because they exist.
- Staging paths are associated with operation IDs.
- Existing game installation directories remain externally owned.
- Original-file backups are preserved independently of mod package archives.
- Garbage collection must respect active profiles, deployments, and unresolved recovery operations.
- Storage relocation is not part of the initial release.

For replacement operations, temporary replacement files should be staged on the destination volume when the chosen replacement primitive requires that. The staging subsystem must account for SSD/HDD placement and sufficient free space.

## 8. File ownership model

Every relevant destination file has an ownership classification.

| State | Meaning | Allowed behavior |
| --- | --- | --- |
| External | File exists but Game Manager does not own it | Inspect; no automatic modification |
| Observed | Potential mod detected | Catalog and analyze |
| Adoption Pending | User requested management | Prepare preservation and validation |
| Managed | File is controlled through a verified deployment manifest | Modify under managed operations |
| Externally Modified | Managed destination no longer matches expected state | Stop destructive changes and request resolution |
| Recovery Required | Ownership or filesystem state cannot be reconciled safely | Restrict further deployment |

Ownership applies to an installation and specific file paths, not globally to every copy of a mod.

### 8.1 First modification of a destination

Before Game Manager first replaces an existing external file:

1. Identify the file and its expected destination.
2. Verify that the deployment recipe permits replacement.
3. Preserve a verified original baseline.
4. Record that baseline durably.
5. Obtain the required user authorization.
6. Apply the change through the deployment engine.

If the original cannot be preserved reliably, the operation must not proceed.

### 8.2 Existing mod adoption

Detection is read-only.

If the user chooses to adopt existing mods:

1. Identify which files belong to the candidate mod.
2. Determine whether another manager owns those files.
3. Show an adoption preview.
4. Preserve the current filesystem state.
5. Establish a verified manifest.
6. Request final approval.
7. Transition only the verified files into managed ownership.

If the application cannot distinguish the mod's files from unrelated game files, adoption remains unavailable or requires a narrower, explicitly reviewed file selection.

### 8.3 Unexpected modifications

If a managed file no longer matches the recorded deployed content, the engine must not silently overwrite or remove it.

Possible resolutions include:

- Preserve the changed file and stop the operation.
- Let the user select a separate backup or quarantine destination.
- Reinspect and adopt the changed state after explicit confirmation.
- Restore a known backup after explaining the consequences.
- Leave the installation in a recovery-required state.

The default response must favor preserving user data over forcing the intended profile state.

## 9. Mod profiles

A mod profile defines the desired mod configuration for one game installation.

Each profile contains:

- Profile identifier and name
- Game installation identifier
- Adapter identifier and version
- Selected mod versions
- Enabled or disabled status
- Priority or load-order information where applicable
- Game-specific installation options
- Activation strategy
- Optional associated launch profile
- Revision identifier

Profile changes are persisted separately from deployed filesystem state.

**Selecting a profile is not equivalent to deploying it.**

A profile may be selected in the interface while its deployment remains pending, failed, or incompatible.

### 9.1 Profile switching

Switching profiles involves:

1. Resolve the current deployed state.
2. Resolve the new profile's desired state.
3. Compare both states.
4. Calculate additions, removals, restorations, and replacements.
5. Check compatibility and conflicts.
6. Preview the differences.
7. Obtain approval.
8. Apply the plan.
9. Verify the result.
10. Mark the new profile active.

Profiles must not assume that all mods require filesystem installation.

For launch-time activation, switching may only change the launch configuration.

### 9.2 Vanilla profiles

A Vanilla profile disables Game Manager-managed mods.

It does not guarantee that the installation matches an official, unmodified copy of the game.

The UI must distinguish these conditions.

## 10. Deployment planning

Deployment planning is a read-mostly operation that calculates intended changes without applying them.

The input includes:

- Game installation identity
- Expected installation revision
- Selected mod profile
- Adapter and compatibility information
- Current deployment manifest
- Current filesystem observations
- Available managed packages
- Backend capabilities

The output includes:

- Files to create
- Files to replace
- Managed files to remove
- Original files to restore
- Conflicts and unresolved decisions
- Required backups
- Estimated disk-space requirements
- Required game shutdown state
- Reversibility and recovery assessment
- Plan fingerprint and expiration conditions

A deployment plan must never be reusable indefinitely.

If relevant files, profile revisions, adapter versions, or installation identity change, the plan must be revalidated.

## 11. Conflict model

### 11.1 File conflicts

A file conflict occurs when multiple selected mods propose incompatible ownership or content for the same effective destination.

For file-overwrite ecosystems, the planner may use a deterministic priority rule to select the winning file.

A conflict is not automatically an error.

The planner must distinguish:

- Identical file content
- Compatible overwrite with defined priority
- Ambiguous overwrite requiring user selection
- Forbidden overwrite
- External ownership conflict

Destination comparisons must account for Windows case-insensitivity and filesystem naming behavior.

### 11.2 Dependency conflicts

Dependency information may describe:

- Required mods
- Optional mods
- Minimum or maximum versions
- Incompatible mod combinations
- Required loaders
- Required game versions

Missing metadata is not proof of compatibility.

The engine must distinguish known validation success from unverified compatibility.

### 11.3 Semantic conflicts

Mods can interfere without changing the same file.

Generic filename-based detection cannot reliably identify these conflicts.

Game adapters or curated compatibility metadata may provide additional validation.

The UI should clearly communicate limitations.

## 12. Deployment operation lifecycle

A deployment follows a durable state machine.

| State | Meaning |
| --- | --- |
| Draft | Proposed operation exists |
| Validated | Plan passes current validation |
| Preparing | Acquiring locks, preserving originals, staging content |
| Prepared | Recovery prerequisites have been verified and persisted |
| Applying | Filesystem changes are in progress |
| Verifying | Actual filesystem state is being checked |
| Active | Required operations verified successfully |
| Recovering | Interrupted or failed operation is being reconciled |
| Restored | Previous verified state has been restored |
| Recovery Required | Automatic recovery cannot proceed safely |
| Failed Before Mutation | Operation failed without changing game files |

The database and filesystem journal must allow the recovery coordinator to distinguish these states after a crash.

A deployment must not transition to Active merely because all copy operations returned success. Verification is a separate requirement.

## 13. Durable filesystem journal

SQLite and the filesystem cannot be treated as one atomic transaction.

A deployment therefore uses two coordinated records:

**SQLite operation record:** Represents the intended operation, associated installation, profile, backend, and overall lifecycle state.

**Filesystem journal:** Records individual planned mutations, original-state references, and observed completion results.

The journal must be written durably before an operation that depends on it for recovery.

### Per-file journal record

A representative record contains:

```
operation_id
step_number
destination_path
operation_kind

expected_original_exists
expected_original_hash
original_backup_ref

expected_new_hash
staged_content_ref

step_state
last_verified_state
error_code
```

Possible step states include Pending, Prepared, Applied, Verified, Restored, and NeedsInspection.

The recovery algorithm must tolerate disagreement between journal state and observed filesystem state.

The actual filesystem, verified against recorded identities and hashes, is essential evidence during recovery.

## 14. Deployment execution workflow

### Step 1 — Acquire exclusive installation lock

No other Game Manager deployment may modify the same installation concurrently.

Use both an in-process coordination mechanism and a Windows-compatible interprocess locking strategy.

Persist operation identity for recovery. A database lock row alone is not sufficient protection against simultaneous processes.

### Step 2 — Verify installation identity

Validate the target path, game installation identity, adapter compatibility, expected profile revision, and running-process state.

Reject stale plans.

### Step 3 — Verify target paths

Check that every destination is contained within approved installation roots under the relevant Windows filesystem semantics.

Reject traversal, unexpected reparse points, unsafe junctions, and unsupported path types.

Resolve and verify paths again at execution time.

### Step 4 — Prepare staged content

Inspect required mod package objects.

Stage replacement content safely, with applicable size limits and disk-space checks.

Avoid unnecessary extraction of unrelated files.

### Step 5 — Preserve original state

Before replacing existing content, preserve recoverable originals and verify their stored references.

An original baseline should be reused across subsequent profile switches rather than being overwritten with whichever modded state happened to be active.

### Step 6 — Persist prepared operation state

Write durable operation intent and step information before modifying game files.

The operation must be discoverable during startup recovery.

### Step 7 — Apply individual mutations

Apply one validated mutation at a time, recording progress at defined checkpoints.

Prefer same-volume replacement operations where appropriate.

Filesystem replacement APIs are selected according to existing-file state, permissions, and required semantics.

Do not assume every file can be replaced while a game or external tool is running.

### Step 8 — Verify

Check intended destination existence, ownership, content hashes or appropriate integrity evidence, and consistency of the resulting manifest.

An adapter may perform additional game-specific checks.

### Step 9 — Commit deployment state

After verification, persist the active profile, deployed manifest, and successful operation state.

Publish a compact completion event.

### Step 10 — Cleanup

Remove temporary files only after they are no longer necessary for recovery.

Preserve original baselines and active package references.

## 15. Recovery algorithm

At application startup, the recovery coordinator inspects unfinished deployment records before permitting further changes to affected installations.

Normal library browsing must remain available during recovery assessment.

### Recovery assessment

For each unfinished operation:

1. Acquire the affected installation lock.
2. Read the durable operation journal.
3. Inspect actual target files.
4. Compare observed files against pre-operation and intended post-operation states.
5. Classify each step as unchanged, applied, restored, externally modified, missing, or ambiguous.
6. Determine whether the operation can safely complete or reverse.
7. Produce a recovery report.
8. Apply authorized or safely automatic recovery actions.
9. Verify the resulting state.
10. Release the lock when the installation reaches a known state.

### Recovery rules

If no filesystem changes occurred, cancel the incomplete operation safely.

If some files were replaced and their originals are verified, attempt controlled restoration where policy permits.

If a file was externally modified after deployment, preserve it and stop rather than overwrite it automatically.

If an original backup is missing or invalid, enter Recovery Required.

If the installation directory is unavailable, defer recovery and prevent new deployment into that installation.

If an operation was already completed on disk but its final database commit was interrupted, verify the deployed manifest and finalize the database record.

Recovery must be repeatable. An interrupted recovery operation must itself be recoverable.

## 16. Safe removal and deactivation

Disabling a mod is not equivalent to deleting its package.

Removing a mod from a profile updates the desired configuration.

The deployment planner calculates which physical files, if any, must be removed or replaced.

Before removing a managed destination, verify that it still matches an expected Game Manager-owned artifact.

If another enabled mod should now provide that destination, replace the current winner through a validated operation.

If no enabled mod should provide the destination, restore the original baseline when one exists, or remove the verified managed file when the original path was absent.

If the destination was externally modified, stop and request resolution.

A mod package remains in storage while any active profile, historical manifest required for recovery, or backup operation references it.

## 17. Link-based deployment

Link-based deployment is a later implementation, but the backend interface must support it.

### Symbolic links

The backend must validate:

- Required privileges or supported Windows configurations
- Target existence and compatibility
- Reparse-point behavior
- Installation and package storage accessibility
- Game compatibility with linked paths

A symbolic link should not be substituted for a normal file unless its behavior is supported by that adapter or verified workflow.

### Hard links

Hard-link deployment has additional restrictions.

Hard links generally require a compatible same-volume filesystem.

Because multiple hard links reference the same underlying file, an in-place edit through the game installation may also alter the linked content.

Therefore:

- Never hard-link directly from the immutable package store to a mutable game destination.
- Use separately managed deployment material where necessary.
- Detect files that a game or loader may modify.
- Prefer copy deployment when immutability cannot be guaranteed.
- Verify filesystem support before offering the strategy.

Link deployment must be presented as an optimization or compatibility-specific option, not an inherently safer replacement for copying.

## 18. Launch-time activation

Some ecosystems activate mods through launch arguments, configuration files, or dedicated loaders.

The Activation Strategy interface handles these cases separately from filesystem deployment.

Examples include:

- Classic Doom source-port WAD and PK3 selections
- Source-engine mod launch configurations
- Game-specific loader invocation
- Per-instance mod-loader arguments

Activation plans may produce:

- Executable identity
- Argument tokens
- Working directory
- Environment overrides
- Required loader version
- Selected content paths
- Pre-launch compatibility checks

Executable arguments must remain structured tokens, not untrusted shell command strings.

The launcher must validate activation readiness before execution.

If a strategy needs to modify a configuration file, that modification must pass through the same ownership and recovery principles as other managed files.

## 19. Existing mod detection and adoption

Initial detection is non-destructive.

An adapter may inspect supported directories, manifests, launcher configurations, or mod-loader metadata.

Detected mods receive confidence and ownership information.

A user may choose among:

- Leave externally managed
- Add to Game Manager's catalog only
- Request adoption into managed storage

Adoption must be a separate operation with its own preview and recovery requirements.

For installations controlled by specialized external managers, Game Manager should initially offer cataloging and launching that manager rather than attempting concurrent ownership.

## 20. Background-job integration

Deployment operations run through the Rust job system.

Recommended job categories:

- Mod archive inspection
- Package hashing and import
- Installed-mod detection
- Conflict analysis
- Deployment preparation
- Deployment execution
- Deployment verification
- Recovery reconciliation

Interactive library requests must never be queued behind expensive mod operations.

Heavy hashing, extraction, and copying should use bounded workers.

Deployment jobs require exclusive installation locks.

Cancellation is permitted at safe checkpoints. It must not terminate an individual critical filesystem mutation halfway through merely to improve responsiveness.

Progress notifications should be throttled and contain small summaries.

Persistent job records must distinguish cancellation before mutation from cancellation requiring recovery.

## 21. Performance requirements

The mod subsystem must not cause a slowdown proportional to every mod in every game during ordinary library browsing.

### Initial targets

- No full mod scan during normal application startup.
- Library summaries retrieve only aggregate mod state when necessary.
- Full mod manifests load only for selected installations or operations.
- Package hashes are cached when safe to reuse.
- Changed packages and destinations are revalidated before destructive operations.
- Mod lists support virtualization.
- Archive extraction runs in bounded background workers.
- HDD-heavy operations are throttled when needed.
- Job progress does not trigger excessive React renders.
- Unrelated games remain browsable during mod processing.

Deployments involving large quantities of files may take substantial time. Responsiveness, safe cancellation, and recoverability are higher priorities than an arbitrary universal completion-time guarantee.

## 22. Security requirements

Mod archives and external metadata are untrusted.

The following controls are mandatory:

- Archive path traversal prevention
- Unexpected absolute-path rejection
- Safe handling of symbolic links and reparse points
- Limits on extracted file count, size, and total expansion
- Unsupported-file-type checks where necessary
- Validation of Windows path syntax and destination roots
- Explicit confirmation for operations replacing external content
- No implicit execution of bundled scripts, DLL installers, or executables
- No arbitrary shell execution through the frontend
- No unrestricted filesystem access through IPC
- No silent privilege escalation
- Verification of target identity immediately before mutation

Security checks must live in Rust, even if the frontend already validates or disables an action.

Game Manager must not attempt to bypass anti-cheat or platform security mechanisms.

## 23. Rust interface boundaries

The implementation should expose conceptual services similar to:

```
trait ModPackageService {
    fn inspect_package(
        &self,
        package: &PackageReference,
    ) -> Result<PackageInspection, ModError>;
}

trait ModProfileService {
    fn resolve_profile(
        &self,
        profile_id: ProfileId,
    ) -> Result<ResolvedProfile, ModError>;
}

trait DeploymentPlanner {
    fn build_plan(
        &self,
        request: DeploymentRequest,
    ) -> Result<DeploymentPlan, ModError>;
}

trait DeploymentBackend {
    fn kind(&self) -> DeploymentKind;

    fn validate(
        &self,
        plan: &DeploymentPlan,
    ) -> Result<ValidationReport, ModError>;

    fn prepare(
        &self,
        plan: &DeploymentPlan,
    ) -> Result<PreparedDeployment, ModError>;

    fn apply(
        &self,
        prepared: &PreparedDeployment,
        journal: &mut DeploymentJournal,
    ) -> Result<DeploymentResult, ModError>;

    fn recover(
        &self,
        operation_id: OperationId,
    ) -> Result<RecoveryReport, ModError>;
}
```

These are interface sketches, not production-ready Rust definitions.

Blocking filesystem operations must run on suitable workers, and deployment execution must receive a lock/operation context guaranteeing correct installation ownership and journaling.

## 24. Tauri command boundaries

Suggested frontend commands:

| Command | Purpose |
| --- | --- |
| `mods_detect_existing` | Inspect an installation without modifying it |
| `mods_inspect_package` | Analyze a selected local package |
| `mods_import_package` | Import a package into managed storage |
| `mod_profiles_list` | Retrieve profiles |
| `mod_profile_save` | Create or update a profile |
| `mod_profile_resolve` | Calculate profile contents and capabilities |
| `deployment_plan` | Produce an immutable preview with a fingerprint |
| `deployment_apply` | Start an authorized background deployment |
| `deployment_status` | Inspect a deployment operation |
| `deployment_verify` | Revalidate managed deployment state |
| `deployment_recovery_plan` | Explain required recovery actions |
| `deployment_recover` | Perform an authorized recovery operation |

The frontend never receives an unrestricted filesystem executor.

The backend retains responsibility for validating operation IDs, expected revisions, paths, ownership, and permissions.

## 25. Required tests

### Filesystem behavior

- Install a mod into an initially empty directory.
- Replace a file while preserving its original.
- Disable a mod and restore the original.
- Switch between two conflicting mod profiles.
- Remove the final managed owner of a destination.
- Attempt deployment to a read-only directory.
- Attempt deployment with insufficient disk space.
- Detect a file locked by another process.
- Handle file paths differing only by case.
- Handle symlinks and junctions inside installation paths.
- Reject malicious archive paths.

### Recovery

- Crash before any filesystem mutation.
- Crash after preserving originals but before deployment.
- Crash after one of several files has been deployed.
- Crash after all files are changed but before final database commit.
- Crash during verification.
- Crash during recovery.
- Encounter a missing original backup.
- Encounter an externally modified managed file.
- Encounter an unavailable installation drive.
- Restart repeatedly with the same unresolved operation.

### Ownership

- Detect existing mods without modifying them.
- Reject unsafe adoption.
- Preserve files owned by another manager.
- Refuse to delete a file whose contents differ unexpectedly.
- Confirm package cleanup never removes active dependencies.

### Performance

- Inspect a package containing thousands of files.
- Resolve profiles containing hundreds of mods.
- Analyze a large conflict set.
- Run package hashing while browsing 15,000 games.
- Verify that unrelated library requests remain responsive.

All destructive-operation tests must use disposable test directories.

## 26. Release acceptance criteria

The initial mod-management feature is ready for release only when:

1. Supported mods can be imported without executing untrusted content.
2. A user can create and select multiple profiles.
3. A deployment preview identifies intended changes.
4. Game Manager preserves original files before managed replacement.
5. A supported deployment can be reversed safely.
6. Conflicting mods are handled according to explicit rules.
7. Interrupted deployments are detected after restart.
8. Recovery never knowingly overwrites unexpected external modifications without permission.
9. Unknown installations or packages are handled conservatively.
10. Background mod operations do not block ordinary library interaction.
11. At least one supported game workflow passes end-to-end integration tests.
12. Known limitations and unsupported deployment features are clearly communicated.

## 27. Implementation milestones

**MDR-01 — Package inventory and inspection**

Implement immutable package storage, safe archive inspection, hashing, and package manifests.

**MDR-02 — Profiles and adapter contracts**

Implement mod profiles, capability declarations, and structured activation plans.

**MDR-03 — Deployment planning**

Implement destination mapping, ownership inspection, conflict detection, and preview generation.

**MDR-04 — Managed copy deployment**

Implement staging, original preservation, journaled file operations, and verification.

**MDR-05 — Recovery engine**

Implement startup reconciliation, rollback, unresolved-conflict handling, and fault-injection tests.

**MDR-06 — Existing-mod adoption**

Implement read-only detection, consent, original-state preservation, and ownership transitions.

**MDR-07 — Launcher integration**

Allow launching supported games with the selected validated mod profile.

**MDR-08 — Initial game compatibility**

Validate representative modding workflows, prioritizing Half-Life 2, classic Doom II, and Stardew Valley.

**MDR-09 — Additional backends**

Add link-based deployment after the managed-copy implementation is stable.

Virtual filesystem support remains a separately planned long-term milestone.

## 28. Decisions for later versions

The following do not need to be resolved before the MVP:

- Full virtual filesystem implementation
- Deep Nexus Mods, Thunderstore, and mod.io integration
- Universal load-order management
- Fully automated installer-script interpretation
- Advanced third-party mod-manager interoperability
- Cross-computer mod-profile synchronization
- Mod collection publishing and sharing
- Native trainer integration
- Complete in-game overlays

These should be considered independent follow-up projects rather than obligations of the first release.

---

**Detailed protocol status:** Recovered proposal under accepted hybrid architecture  
**Code implementation:** Not started  
**Next step:** Validate the architecture through focused implementation prototypes and finalize the initial game adapters.

## 29. Confirmed target matrix and implementation status

| Target | Intended adapter scope | Sequencing |
| --- | --- | --- |
| Half-Life 2 | Source engine, sourcemods, separate mod roots and launch profiles | First candidate set |
| Classic Doom II (not the reboot) | WAD/PK3 selection and source-port launch arguments | First candidate set; launch-time activation test |
| Stardew Valley | SMAPI manifests, dependencies, mod directories, loader launch | First candidate set |
| Minecraft | Versioned instances; Fabric, Forge, NeoForge | Incremental specialized support |
| Cyberpunk 2077 | Multiple frameworks and specialized target paths | Incremental specialized support |
| Skyrim | Plugins, load order, archives and compatibility rules | Incremental specialized support |
| Baldur's Gate 3 | Package mods, load-order metadata, native mod support | Incremental specialized support |
| Terraria | tModLoader integration and its existing facilities | Incremental specialized support |

These are confirmed product targets, not implemented compatibility claims. The user's list does not establish an immutable implementation order among all eight games. The first candidate set is Half-Life 2, classic Doom II, and Stardew Valley; concrete editions, builds, loader versions, mod samples, and the final MVP adapter subset require prototype validation. Do not substitute an unspecified Doom reboot or silently select a Skyrim/Minecraft edition.

Managed copy is the first filesystem backend, supported launch-time activation belongs in the initial architecture, links follow after copy/recovery stability, and virtual filesystem deployment is long-term. Cataloging and manual installation recipes should remain useful without a dedicated game adapter; unsafe or unknown deployment stays disabled. Delegating to a specialized external manager can be considered per game, but is not a replacement for the integrated MVP mod engine or permission to take over another manager's files.

## 30. Additional retained safety requirements

Reject absolute archive paths, traversal, Windows device-name/alternate-stream ambiguities, case-insensitive destination collisions, unsafe archive links, oversized expansion, and unsupported formats. Define explicit file-count, byte and expansion limits during implementation. Validate destination containment through actual filesystem identity and reparse-point handling at mutation time; a textual prefix check is insufficient.

Detect stale plans after profile edits, game/store updates, adapter changes, drive moves, and external-manager activity. Revalidate under the installation lock. Do not silently change deployment strategy after permission or compatibility failure. Never hardlink immutable package objects directly into game-writable destinations. A future hardlink backend needs disposable copies or another proven isolation policy.

Intent is persisted before mutation. The recovery coordinator must inspect disk even when the journal reports only intent, since a crash can occur after the write but before completion was recorded. A failed operation is not necessarily reverted. Recovery steps must themselves be journaled, repeatable and verified. Single-file replacement does not provide whole-deployment atomicity.

After adoption, a preserved baseline proves the pre-adoption state only. It must never be labeled as the original vendor file without evidence. Missing originals, external edits or unavailable drives require preservation and an explicit recovery report. Tests must include recovery interrupted twice and restoration against a database snapshot that disagrees with disk.

No real game installation has been modified, no backend has been implemented, and no deployment/recovery fixtures have passed. This is the recovered proposed detailed protocol under accepted architectural and safety constraints.
