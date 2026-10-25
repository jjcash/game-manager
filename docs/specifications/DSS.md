# Game Manager

Revision: reconciled 2026-10-25 against the original [Game Manager Annotations Review](https://chatgpt.com/c/6add59c2-80bc-83ea-a64e-e890c3f606f6) and later user decisions. See [source and decision register](../SOURCE_AND_OPEN_DECISIONS.md).

**Implementation status: not started.** Specifications, interface sketches, SQL examples, and acceptance targets describe intended behavior; they are not existing code or verified results. Later explicit decisions override earlier drafts: installed Windows only, no portable distribution, no Playnite migration requirement, and external trainers first in later Game Tools work.

## Database & Storage Design Specification — Reconciled v0.2

**Status:** Proposed for architectural acceptance  
**Development status:** Not started  
**Target:** Windows desktop, local-first  
**Database:** SQLite  
**Access layer:** Rust + SQLx  
**Future compatibility:** Android companion, game assistance tools, optional synchronization

---

## 1. Scope and objectives

This specification defines the persistent data model and storage responsibilities for Game Manager.

It covers:

- Game identities and library entries
- Multiple installations and configurable grouping
- Game metadata and external provider identifiers
- Launch configurations and activity tracking
- Game resources, notes, and future integrations
- Mod inventory, packages, profiles, and deployments
- Filesystem ownership, recovery, and backups
- Background-job persistence
- Installed application data directories and configurable storage roots
- Search indexes, performance requirements, and schema migrations

The design must support at least 15,000 library entries for initial performance validation and 25,000 entries during stress testing.

It must remain functional without an internet connection.

## 2. Data ownership

The database is the authoritative source of application-managed structured information.

Game installations, ROMs, externally managed mods, and user-selected files remain external resources unless an explicit operation establishes management responsibilities.

The application will distinguish four data ownership categories:

| Category | Examples | Policy |
| --- | --- | --- |
| Application-owned | SQLite database, settings, indexes | Game Manager may modify |
| Managed content | Imported mod archives, thumbnails | Game Manager controls lifecycle |
| Managed deployment | Files explicitly installed through Game Manager | Modification only through validated operations |
| External content | Games, ROMs, manually installed mods | Inspect without assuming ownership |

Removing a game from the library must not uninstall the game or delete its personal files.

Removing a mod from the inventory must not automatically delete externally owned files.

## 3. Identity model

The application distinguishes these entities:

**Game:** A canonical game identity, independent of storefront and installation.

**Library Entry:** A user's association with a game from a particular source or manually created collection.

**Game Installation:** One installed copy of a game, including its local path, edition, version when known, and platform.

**Launch Profile:** A launch configuration associated with an installation, such as Vanilla, Modded, or a custom command-line profile.

**Mod Profile:** A configuration of enabled mods and versions for one installation or compatible installation context.

Every primary entity will have an internal stable identifier independent of third-party services.

Proposed implementation choice: use UUIDv7 identifiers encoded as text at domain boundaries, with INTEGER row IDs for search indexes or internal tables where advantageous.

External provider IDs will be stored separately and namespaced by provider.

## 4. Library schema

### 4.1 Core entities

| Table | Important columns | Responsibility |
| --- | --- | --- |
| `games` | id, title, sort\_title, release\_date, created\_at, updated\_at | Canonical game records |
| `game_aliases` | id, game\_id, title, alias\_type | Alternative and localized names |
| `game_external_ids` | game\_id, provider\_id, external\_id | Provider identity mapping |
| `library_sources` | id, source\_type, display\_name, configuration\_ref | Steam, GOG, manual, etc. |
| `library_entries` | id, game\_id, source\_id, external\_item\_id, status, added\_at | Individual library memberships |
| `game_installations` | id, entry\_id, root\_path\_ref, platform, version, detection\_state | Installed copies |
| `launch_profiles` | id, installation\_id, title, launcher\_kind, executable\_ref, argument\_data, working\_dir\_ref | Game launch configurations |
| `game_resources` | id, game\_id, resource\_type, title, URI, origin | Maps, guides, wikis, tools |
| `game_notes` | id, game\_id, title, body, created\_at, updated\_at | Personal notes |
| `play_sessions` | id, installation\_id, launch\_profile\_id, started\_at, ended\_at, duration\_seconds, confidence | Locally recorded activity |
| `imported_playtime` | id, entry\_id, source\_id, seconds, observed\_at | Historical source-reported playtime |

### 4.2 Metadata and organization

| Table | Purpose |
| --- | --- |
| `metadata_providers` | Provider identities and capabilities |
| `game_metadata_values` | Source-specific values and provenance |
| `game_field_overrides` | User-defined overrides |
| `tags` | Tag definitions |
| `game_tags` | Game-to-tag assignments |
| `genres` | Genre definitions |
| `game_genres` | Game-to-genre assignments |
| `collections` | User-defined game collections |
| `collection_entries` | Collection memberships |
| `game_assets` | Artwork references and origin metadata |
| `game_ratings` | Personal, community, and critic ratings with source information |

The effective value of a metadata field is resolved by a defined policy:

1. Explicit user override, when present.
2. Preferred provider value, when available.
3. A fallback provider value.
4. Empty or unknown.

The application must not overwrite user edits during metadata refreshes.

Community and critic scores are provider-attributed imported ratings, not scores calculated by Game Manager.

### 4.3 Multiple installations

A game may have multiple library entries and installations.

Library grouping is a query and presentation decision, not a destructive consolidation of database records.

Grouping or separating installations must not alter ownership, playtime history, mod profiles, or the identity of individual installed copies.

Each launchable installation may have multiple launch profiles.

## 5. Search and Details View storage model

The primary library interface is a virtualized Details View.

The database must support efficient retrieval of small result sets containing only required columns.

A library summary projection should include:

- Game and library entry identifiers
- Display title and sort title
- Source and platform summaries
- Installation availability
- Completion status
- Personal rating
- Total known playtime
- Last played timestamp
- Cover thumbnail asset identifier
- Aggregate mod status, when available

Full descriptions, all mod files, notes, and complete artwork metadata will not be retrieved during ordinary library browsing.

### 5.1 Full-text searching

SQLite FTS5 will index searchable titles and selected metadata.

An external-content FTS5 index is preferred, backed by a maintained search-document table.

The search index must be updated when relevant metadata changes and must support validation and rebuilding.

Structured filters such as platform, installation state, completion status, and personal rating will be implemented through ordinary indexed SQL queries.

### 5.2 Sorting and pagination

Use database-side filtering and sorting.

Prefer keyset pagination for stable, commonly used sort orders. Other navigation methods may be implemented for arbitrary scroll-position jumps.

Every paginated query must use deterministic ordering and a stable tie-breaker.

Cursors must be invalidated or restarted when their underlying query or relevant library revision changes.

The frontend must not be required to load every game to perform sorting.

### 5.3 Aggregate data

Frequently used aggregate values, such as total playtime and mod count, may be cached in maintained summary tables.

Aggregate caches are derived data, not the authoritative source.

The application must be able to rebuild them from normalized records.

## 6. Mod-management schema

The modding subsystem will use the accepted hybrid architecture.

### 6.1 Inventory and profiles

| Table | Important columns | Responsibility |
| --- | --- | --- |
| `mods` | id, title, description, primary\_source\_id | Logical mod identity |
| `mod_versions` | id, mod\_id, version\_label, package\_id | Individual versions |
| `mod_packages` | id, content\_hash, archive\_ref, format, byte\_size | Immutable imported packages |
| `mod_package_files` | package\_id, relative\_path, hash, byte\_size | Package contents |
| `mod_external_ids` | mod\_id, provider\_id, external\_id | External repository references |
| `mod_installation_records` | installation\_id, mod\_id, ownership\_state, detected\_at | Association with a game installation |
| `mod_profiles` | id, installation\_id, name, adapter\_id, created\_at | Mod configuration profiles |
| `mod_profile_entries` | profile\_id, mod\_version\_id, enabled, priority, options\_data | Profile contents |
| `mod_dependencies` | mod\_version\_id, target\_mod\_id, constraint\_data, dependency\_type | Dependency metadata |
| `mod_adapter_settings` | installation\_id, adapter\_id, schema\_version, settings\_data | Game-specific adapter configuration |

Mod packages and installed game files are different objects.

A package can exist in the library without being deployed.

A mod may be marked as observed, awaiting adoption, or managed for a particular installation. These states must not automatically apply to every installation of the same game.

Profile contents must preserve the chosen version of each mod.

### 6.2 Deployment and recovery

| Table | Purpose |
| --- | --- |
| `mod_deployments` | Deployment identity, installation, profile, backend, state |
| `deployment_operations` | Planned and completed filesystem actions |
| `deployment_file_records` | Managed destinations, hashes, ownership, source references |
| `original_file_backups` | Preservation of pre-deployment files |
| `deployment_recovery_records` | Recovery attempts and unresolved operations |
| `installation_locks` | Persistent operation ownership and recovery information |

The authoritative operation journal must preserve enough information to reconcile the database and filesystem after interruption.

The database should record deployment intent before irreversible filesystem changes occur.

Each operation must be idempotent or explicitly recoverable.

Only one deployment may modify a particular game installation at a time.

A deployment plan is bound to a particular installation revision. Unexpected external changes require revalidation.

### 6.3 Deployment strategies

The schema must distinguish:

- Managed file-copy deployment
- Symbolic-link deployment
- Hard-link deployment
- Launch-time activation
- Future virtual filesystem deployment

Launch-time activation is not inherently a filesystem deployment.

Game adapters determine which strategies are supported.

The database records the selected strategy and the exact versions and rules used to activate a profile.

## 7. Mod package storage

Mod packages will be stored in a dedicated managed library outside game installations.

Recommendation: immutable, content-addressed archive storage.

A cryptographic hash, such as SHA-256, identifies package content. Two identical archives may share a single stored object.

Package metadata remains in SQLite.

Extraction and deployment staging directories are separate from the immutable archive store.

A stored package must never be edited in place.

The application will use reference tracking and a delayed garbage-collection process to remove unused managed objects.

An object must not be deleted while a profile, deployment, backup, or recovery record still depends on it.

## 8. Filesystem path model

The application must distinguish managed relative paths from external absolute paths.

A path reference can include:

- Storage root identifier
- Relative path or external location
- Expected object type
- Last-known filesystem identity
- Existence and verification state

Managed storage paths are resolved relative to a configured application data root.

External installation paths must be validated before each privileged operation.

Windows drive-letter changes, junctions, symbolic links, and filesystem capabilities must be considered.

Path normalization alone is not proof that an operation is confined to an allowed directory.

## 9. Application storage directories

### 9.1 Installed mode

The installed application should default to appropriate per-user Windows directories for configuration, local data, cache, and logs.

Proposed logical structure:

| Location | Contents |
| --- | --- |
| `config/` | Preferences and storage configuration |
| `database/` | Main SQLite database and associated WAL files |
| `media/originals/` | Artwork originals |
| `media/cache/` | Generated thumbnails |
| `mods/packages/` | Immutable mod archives |
| `mods/staging/` | Temporary extraction and deployment preparation |
| `mods/manifests/` | Optional human-readable manifest exports |
| `backups/database/` | Database snapshots |
| `backups/deployments/` | Preserved original game files |
| `journals/` | Filesystem operation recovery records |
| `logs/` | Application diagnostics |
| `tmp/` | Disposable temporary files |

This layout is logical, not a requirement that every directory share one physical drive.

### 9.2 Installed-only policy

Portable distribution was dropped by explicit user decision after this draft. There is no portable-mode data root, relocation framework, or portable-specific release/test requirement. Centralized path management and configurable installed data locations remain. Binaries, durable database/settings, regenerable cache, packages, and recovery assets must remain distinct.

### 9.3 Storage configuration

Users should eventually be able to choose:

- Database location
- Artwork cache location
- Mod archive library
- Temporary staging location
- Backup location

The MVP may expose only the most important settings.

Changes to managed storage roots must use a validated migration operation rather than simply updating a path string.

## 10. Database operation policy

SQLite will operate locally with WAL enabled on compatible filesystems.

Foreign-key enforcement must be enabled for every connection.

Recommended initial configuration:

- One coordinated write connection or write path
- A small bounded read pool
- Short transactions
- Prepared parameterized statements
- Busy-timeout handling
- Explicit migration initialization
- Idle or controlled WAL checkpoint management

Database reads should not remain open while expensive filesystem or network work is performed.

Database connection limits and checkpoint frequency will be tuned through performance benchmarks.

The active SQLite database must not depend on network filesystem access.

## 11. Transaction boundaries

### 11.1 Game metadata import

1. Fetch and normalize external data outside a database transaction.
2. Match canonical identities.
3. Validate records and resolve expected conflicts.
4. Commit a bounded batch using a database transaction.
5. Record imported source identifiers and update relevant indexes.
6. Schedule artwork processing separately.
7. Publish a library revision event after commit.

A failed batch must not partially update related records.

### 11.2 Mod deployment

Filesystem deployment spans multiple resources and cannot be made fully atomic by a SQLite transaction.

Use durable operation intent, original-file preservation, per-file verification, and recovery-aware state transitions.

Database state must not claim successful deployment until the filesystem has been verified.

### 11.3 Deleting library entries

Library deletion is distinct from uninstalling a game.

Before deleting a record, identify dependent profiles, sessions, notes, installations, and managed content.

Prefer explicit user-facing choices over silently cascading deletion of valuable personal data.

No deletion operation may remove an external game installation by default.

## 12. Backup and recovery

### 12.1 Database backups

Use SQLite's supported online backup mechanism or another verified consistent snapshot method.

Never assume that copying a live WAL-mode database file alone produces a consistent backup.

Every backup should record:

- Backup format version
- Application version
- Database schema version
- Creation timestamp
- Integrity information
- Included data categories

### 12.2 Application backups

The backup system should eventually support:

- Database-only backup
- Database plus managed configuration and metadata
- Complete application-managed data backup, including selected assets and mod archives

External games and ROM libraries are excluded unless explicitly selected.

A complete backup requires a consistent snapshot of both database references and included filesystem objects.

### 12.3 Recovery

On startup, the application checks for unfinished deployment operations.

Unfinished operations must be reconciled before another deployment affects the same installation.

Recoverable cases should be presented clearly to the user.

The application must never assume that a deployment is complete merely because a database status says it was previously running.

## 13. Persistent jobs

The `background_jobs` table will store:

- Job identifier and type
- State and priority
- Creation and execution timestamps
- Serialized, versioned parameters
- Progress checkpoints
- Retry count
- Error state
- Associated game or installation
- Recovery requirements

Jobs must have explicit restart policies.

Read-only discovery and metadata imports may resume from checkpoints when supported.

Filesystem-changing jobs require recovery reconciliation before retrying.

Completed job history can be pruned according to a retention policy, but unresolved recovery data must be preserved.

## 14. Game Tools compatibility

The initial `game_resources` table will support categories such as:

- Website
- Wiki
- Guide
- Interactive map
- Static map
- External trainer
- External utility
- Community resource

Resources may be associated with a canonical game and optionally restricted to a specific installation or game version.

No executable will be automatically trusted merely because its link is stored in the database.

Future trainer support will proceed in stages:

1. External tool registry and launching
2. External trainer integration and presets
3. Native capabilities for specifically supported offline games, subject to a separate security review

Future interactive maps may require additional entities for maps, layers, markers, coordinates, personal annotations, and completion progress.

These tables will be introduced when that subsystem is developed. They are not required in the initial migration.

## 15. Data integrity and maintenance

Database maintenance must include:

- Foreign-key consistency checks
- SQLite integrity verification
- FTS index consistency checks
- Missing external-file detection
- Managed-content reference reconciliation
- Mod manifest verification
- Stale temporary-file cleanup
- Safe recovery-journal retention

Maintenance must run through bounded background jobs, not as a mandatory full scan during startup.

Caches and search indexes should be rebuildable.

Personal notes, manually entered game records, play sessions, mod profiles, and recovery originals are not disposable caches.

## 16. Migration strategy

Every schema change must be versioned.

Migrations run before normal database access is enabled.

For migrations with meaningful data-loss risk:

1. Verify available storage.
2. Create a consistent pre-migration backup.
3. Apply changes transactionally where possible.
4. Validate constraints and expected transformations.
5. Record the schema version.
6. Enable normal application operation.

An unsupported newer schema should cause a controlled compatibility error instead of attempting an unsafe downgrade.

Mod manifest and job payload formats may require independent versioning from SQLite schema migrations.

## 17. Performance acceptance criteria

The storage layer will be tested with 500, 2,000, 5,000, 12,320, 15,000, and 25,000 game records.

Required measurements:

- Database initialization
- Indexed search
- Sorting and filtering
- Page retrieval
- Detail retrieval
- Import batch throughput
- Mod inventory retrieval
- Deployment manifest processing
- Backup creation and validation
- HDD versus SSD performance
- WAL checkpoint behavior under concurrent work

Initial targets include p95 indexed search under 100 ms and small library-page retrieval under 50 ms on reference hardware.

These are engineering goals, not verified performance claims.

## 18. Implementation sequence

**DSS-01:** Create core game, source, library entry, and installation migrations.

**DSS-02:** Implement launch profiles and library summary queries.

**DSS-03:** Implement search documents, FTS5 synchronization, and indexes.

**DSS-04:** Establish media storage and managed asset references.

**DSS-05:** Create persistent jobs and import checkpoints.

**DSS-06:** Implement mod inventory, packages, versions, and profiles.

**DSS-07:** Implement deployment manifests, recovery journal, and backup references.

**DSS-08:** Validate installed storage, configurable roots, and mixed SSD/HDD placement; no portable mode.

**DSS-09:** Execute integrity, backup, and performance tests.

## 19. Acceptance gates

Release readiness requires the following future evidence; these are validation gates, not work already completed:

- The identity model is translated into tested migrations.
- Cross-table ownership rules are explicit.
- Details View queries are validated with realistic datasets.
- Mod deployment can be reconciled after interruption.
- External files are never treated as application-owned without authorization.
- Database backups can be restored successfully.
- Paths are validated on Windows and across storage volumes.
- Schema and manifest migrations are versioned.
- Future Game Tools integrations can be added without altering canonical game identity.

---

This document defines the intended architecture. No database migrations, executable code, or filesystem layout have been implemented yet.

## 20. Reference SQL recovered from the original discussion

The following examples were supplied with the original DSS. They are incomplete, untested design-reference SQL, not migrations and not authorization to create a database. Reconcile `created_at_ms` and other example column names with the conceptual tables before implementation. Foreign keys must be enabled on every connection. Source/item uniqueness does not justify merging different canonical games by title.

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE games (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    release_date TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE library_sources (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,
    display_name TEXT NOT NULL
);

CREATE TABLE library_entries (
    id TEXT PRIMARY KEY,
    game_id TEXT NOT NULL
        REFERENCES games(id) ON DELETE RESTRICT,
    source_id TEXT
        REFERENCES library_sources(id),
    external_item_id TEXT,
    completion_status TEXT NOT NULL
        DEFAULT 'not_started',
    added_at_ms INTEGER NOT NULL,
    CHECK (
        source_id IS NOT NULL
        OR external_item_id IS NULL
    )
);

CREATE UNIQUE INDEX ux_library_source_item
ON library_entries(source_id, external_item_id)
WHERE source_id IS NOT NULL
  AND external_item_id IS NOT NULL;

CREATE INDEX ix_library_game
ON library_entries(game_id);

CREATE INDEX ix_library_completion
ON library_entries(completion_status);

CREATE INDEX ix_games_sort
ON games(sort_title, id);

CREATE TABLE game_installations (
    id TEXT PRIMARY KEY,
    entry_id TEXT NOT NULL
        REFERENCES library_entries(id)
        ON DELETE RESTRICT,
    root_path_ref TEXT NOT NULL,
    platform TEXT,
    version TEXT,
    detection_state TEXT NOT NULL
        DEFAULT 'unknown'
);

CREATE INDEX ix_installations_entry
ON game_installations(entry_id);

CREATE TABLE launch_profiles (
    id TEXT PRIMARY KEY,
    installation_id TEXT NOT NULL
        REFERENCES game_installations(id)
        ON DELETE RESTRICT,
    title TEXT NOT NULL,
    launcher_kind TEXT NOT NULL,
    executable_ref TEXT,
    arguments_json TEXT NOT NULL DEFAULT '[]'
        CHECK (json_valid(arguments_json)),
    working_directory_ref TEXT
);

CREATE INDEX ix_launch_profiles_installation
ON launch_profiles(installation_id);
```

```sql
CREATE TABLE game_search_documents (
    id INTEGER PRIMARY KEY,
    game_id TEXT NOT NULL UNIQUE
        REFERENCES games(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    aliases TEXT NOT NULL DEFAULT ''
);

CREATE VIRTUAL TABLE game_search_fts
USING fts5(
    title,
    aliases,
    content='game_search_documents',
    content_rowid='id'
);

CREATE TRIGGER search_documents_insert
AFTER INSERT ON game_search_documents
BEGIN
    INSERT INTO game_search_fts(
        rowid, title, aliases
    )
    VALUES (
        new.id, new.title, new.aliases
    );
END;

CREATE TRIGGER search_documents_delete
AFTER DELETE ON game_search_documents
BEGIN
    INSERT INTO game_search_fts(
        game_search_fts, rowid, title, aliases
    )
    VALUES (
        'delete', old.id, old.title, old.aliases
    );
END;

CREATE TRIGGER search_documents_update
AFTER UPDATE ON game_search_documents
BEGIN
    INSERT INTO game_search_fts(
        game_search_fts, rowid, title, aliases
    )
    VALUES (
        'delete', old.id, old.title, old.aliases
    );

    INSERT INTO game_search_fts(
        rowid, title, aliases
    )
    VALUES (
        new.id, new.title, new.aliases
    );
END;
```

## 21. Ownership, retention and restoration clarifications

- A canonical Game is distinct from LibraryEntry, GameInstallation, LaunchProfile and ModProfile. Grouping is a presentation preference; it must not collapse installation identity or share mod ownership implicitly.
- User overrides, imported provider metadata, personal scores, imported community/critic scores, local sessions and imported playtime retain distinct provenance. Avoid double-counting provider totals and local sessions.
- A mod profile is desired state; a deployment and its file records describe observed, verified state. The active profile changes only after verification, or validated launch-time activation when no writes are needed.
- Original baselines, active manifests, unfinished journals and referenced packages are recovery-critical data, never cache. Retention or database pruning must preserve their references. A missing backup is an integrity error, not a reason to delete the destination.
- Database restore cannot restore game files. Reconcile operation records, journals, manifests, installation identities, managed packages, baseline backups and actual destinations before further mutation. Preserve a failed database; never silently recreate it.
- Paths are references through the storage service. The logical directory examples in DSS and MDRS describe storage classes, not two conflicting hardcoded physical trees. A final shared root map and filename policy will be selected during implementation. Destination-volume staging may be required for replacement.
- Initial configurable storage selection is retained; relocating active mod storage or moving data between computers needs a separately validated migration workflow and is outside the MVP. The active database uses a compatible local filesystem.
- FTS5, WAL, coordinated writes, small read pool, hash format, concrete columns, indexes and retention limits remain design choices to test. The original DSS proposed them; this is not missing historical content or evidence of implementation.

## 22. Verification retained from the existing package

Required future fixtures cover migration upgrades and interrupted migrations, foreign-key/uniqueness constraints, refresh-preserved overrides, 15,000-game bounded queries, consistent live backup/restore, cache loss versus recovery-asset loss, restored-database/divergent-filesystem reconciliation, unavailable drives, and repeated recovery. Test query plans and checkpoint behavior on SSD and HDD. No migration, benchmark or recovery test has been executed for an application.
