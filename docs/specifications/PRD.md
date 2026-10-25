# Game Manager

Revision: reconciled 2026-10-25 against the original [Game Manager Annotations Review](https://chatgpt.com/c/6add59c2-80bc-83ea-a64e-e890c3f606f6) and later user decisions. See [source and decision register](../SOURCE_AND_OPEN_DECISIONS.md).

**Implementation status: not started.** Specifications, interface sketches, SQL examples, and acceptance targets describe intended behavior; they are not existing code or verified results. Later explicit decisions override earlier drafts: installed Windows only, no portable distribution, no Playnite migration requirement, and external trainers first in later Game Tools work.

## Product Requirements Document — Reconciled v0.2

**Project:** Game Manager  
**Type:** Free and open-source game library, launcher, and mod manager  
**Primary platform:** Windows  
**Architecture:** Local-first desktop application  
**Status:** Initial requirements baseline  
**Date:** October 25, 2026

---

## 1. Product vision

Game Manager is a comprehensive, high-performance application for organizing, launching, customizing, and tracking video games.

The application aims to combine:

- Large-scale game library management
- Multi-platform game discovery and launching
- Integrated, cross-game mod management
- Personal gaming records and organization
- Metadata and achievement integrations
- Customizable interfaces and a fullscreen experience
- Optional mobile companion functionality

Performance, reliability, data ownership, and visual quality are foundational product requirements.

## 2. Design principles

1. **Performance first:** Collections containing thousands of games must remain responsive.
2. **Local first:** Core functionality works without accounts or internet access.
3. **Non-destructive:** Operations involving game installations, mods, and personal data prioritize safety and reversibility.
4. **Modular:** Internal services, game adapters, and deployment backends remain independently maintainable.
5. **User control:** Automation should be available without removing manual control.
6. **Visual quality:** A coherent, polished interface with room for increasingly elaborate presentation.
7. **Open source:** Community contributions are encouraged, with third-party extensions planned for future versions.

## 3. Target environment

Initial development and performance reference hardware:

- CPU: AMD Ryzen 5 4500
- Memory: 64 GB RAM
- Operating system: Windows
- Existing game library: 12,320 entries
- Storage: Games primarily on HDD; application may be installed on SSD
- Preferred library interface: Details View

Initial release testing shall include datasets of 500, 2,000, 5,000, 12,320, 15,000, and 25,000 games.

The application should not require 64 GB of RAM for normal operation. Additional lower-memory reference configurations will be established.

## 4. Initial release scope

### P0 — Required for the first usable release

**Library management**

- Add, edit, and remove game records.
- Import games from supported platforms.
- Display games in a fast, virtualized Details View.
- Provide a virtualized cover-grid view.
- Search, filter, and sort library entries.
- Display game artwork and essential metadata.
- Support multiple installations of one game, with configurable grouping.
- Preserve manual metadata overrides.

**Game launching**

- Configure executable paths and launch arguments.
- Launch games from supported platforms and standalone installations.
- Support multiple launch configurations.
- Record local play sessions where reliable process tracking is possible.

**Mod management**

- Catalog mods for individual game installations.
- Import local mod archives and directories.
- Create, duplicate, rename, and switch mod profiles.
- Enable and disable managed mods.
- Detect existing mods without immediately taking ownership.
- Request explicit approval before adopting existing files into managed storage.
- Validate deployment paths and conflicts.
- Provide reversible, manifest-driven deployment.
- Support game-specific activation rules.
- Implement a limited initial set of tested game adapters.

**Reliability**

- Maintain a versioned SQLite database.
- Support library export and import.
- Provide database backup and recovery.
- Preserve deployment operation journals.
- Recover safely from interrupted supported operations.

**Interface**

- Provide one polished dark theme.
- Support keyboard and mouse navigation.
- Show operation progress, errors, and recovery actions.
- Keep the UI responsive during background jobs.

### P1 — Important post-MVP capabilities

- Broader storefront imports.
- Additional modding adapters.
- Mod repository integrations.
- Improved dependency and compatibility checking.
- Link-based mod deployment.
- Expanded personal notes and game organization.
- Game activity statistics.
- Achievement integrations.
- Basic controller-friendly fullscreen interface.

### P2 — Long-term capabilities

- Advanced customizable dashboards.
- Extensive themes and theme customization.
- Personalized website-inspired theme.
- Advanced fullscreen interface with trailers, music, and achievements.
- Android companion.
- Optional cloud synchronization.
- Virtual filesystem deployment for supported games.
- Community extension ecosystem and extension marketplace.
- Additional emulator and ROM-library integrations.

## 5. Platform integration priorities

Integrations should be implemented in the following priority order:

1. Steam
2. GOG
3. Epic Games
4. Ubisoft Connect
5. EA App
6. Amazon Games
7. Emulators, including RomM, RetroArch, and RetroAchievements
8. Battle.net
9. itch.io
10. Standalone executables

This order represents product priority, not a guarantee of implementation feasibility or release order.

Standalone executable launching is an MVP requirement despite its lower position in the discovery integration list.

Each integration must document supported capabilities, authentication requirements, limitations, and failure handling.

## 6. Mod-management architecture

The mod manager will use a universal, capability-based engine with game-specific adapters.

Supported activation and deployment approaches:

- Managed file copying
- Symbolic or hard-link deployment
- Launch-time mod activation
- Game-specific loaders and package-management integrations
- Virtual filesystem deployment in future versions

Managed copy deployment is the first implemented filesystem backend.

Adapters define supported installation paths, mod formats, activation rules, dependencies, and compatibility checks.

The first representative targets should include Half-Life 2 and Stardew Valley, with classic Doom II as an early launch-time activation test case. The exact initial adapter set will be confirmed after technical prototypes.

Complex games such as Skyrim, Minecraft, Cyberpunk 2077, and Baldur's Gate 3 will receive more specialized support incrementally.

Unknown or unsupported mod installations must never be modified automatically.

## 7. Performance requirements

The following are provisional engineering acceptance targets, to be measured on reference hardware in optimized release builds:

- Startup: Fast application-shell rendering with no mandatory library-wide scan.
- Initial library: Aim for interactivity within one second after window initialization with warm local data.
- Search: Typical indexed queries should return within 100 ms.
- Filtering and sorting: Typical indexed operations should remain within 150 ms.
- Scrolling: Target 60 FPS with no progressive slowdown.
- Imports: UI interaction must remain responsive.
- Memory: Cache usage must be bounded, with no sustained growth during repeated library navigation.

Separate performance tests will cover cold starts, HDD and SSD storage, artwork loading, simultaneous imports, and reduced-memory systems.

The 15,000-game dataset is the initial release-scale acceptance test; 25,000 games is the stress-test target.

## 8. Interface requirements

The first release will use a consistent dark visual system.

Initial navigation:

- Library
- Game Details
- Mods
- Settings

The game detail page will provide access to launch configurations, metadata, and mod profiles.

The interface architecture should support future dashboards, richer statistics, and fullscreen presentation without requiring a complete redesign.

Visual polish is an important long-term product goal, but elaborate animations and custom themes must not compromise responsiveness.

## 9. Data and installed storage

SQLite will store structured game, library, profile, and activity data.

Artwork, original mod archives, cached thumbnails, and backup files will be stored separately.

Game Manager shall support configurable data locations, including SSD-based application data with game installations on different drives.

Only an installed Windows distribution is in scope. Portable distribution, portable-mode detection, relocation workflows, and portable-specific testing were dropped. Configurable installed data locations remain required.

Cloud services are optional and must not be required for core library or mod-management functionality.

## 10. Explicitly excluded from the initial MVP

- Full Playnite data migration
- Android synchronization
- Public community rating infrastructure
- Third-party extension marketplace
- Advanced custom-theme editor
- Universal compatibility with every game mod
- Full virtual-filesystem deployment
- Comprehensive in-app ROM management
- Replacement of RetroArch or RomM
- Automatic management of files owned by other mod managers

## 11. First usable MVP acceptance workflow

The first usable MVP is successful when a user can complete the workflow below. This is the combined outcome of roadmap M0–M5; M0 alone is a performance prototype.

1. Open Game Manager and browse a large library without noticeable progressive slowdown.
2. Import games from the first supported source.
3. Find and launch a game.
4. Open that game's Mods section.
5. Import a supported mod.
6. Create or select a mod profile.
7. Preview, apply, and safely reverse a supported deployment.
8. Continue browsing the library during noncritical background processing.
9. Close and reopen the application without unnecessary full-library processing or loss of data.

All destructive or potentially destructive operations must provide explicit safeguards.

## 12. Future specification documents

The PRD will be followed by:

1. Technical Architecture Specification
2. Database and Storage Specification
3. Mod Deployment and Recovery Specification
4. UI/UX Design Specification
5. Performance Benchmark Plan
6. Implementation Roadmap and Issue Backlog

These documents will reference this PRD and record subsequent decisions through versioned architecture decision records.

## 13. Confirmed compatibility targets


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


## 14. Integration capability requirements


1. Steam
2. GOG
3. Epic Games
4. Ubisoft Connect
5. EA App
6. Amazon Games
7. Emulators — RomM, RetroArch, RetroAchievements; other emulators later
8. Battle.net
9. itch.io
10. Standalone executables

This is the exact product priority order. It does not require every integration before the MVP. Manual entry and standalone executable launching remain MVP requirements despite standalone discovery being last.


For each adapter, distinguish owned-library import, installed-game detection, metadata, launching, activity, and achievements. Document authentication, rate limits, offline behavior, provenance, supported identifiers, and unavailable-service behavior separately. A provider name does not promise all capabilities.

RomM remains the ROM library manager; Game Manager may display its collection without moving or duplicating ROMs. RetroArch remains the emulator. RetroAchievements is a separate achievement integration. Game Manager is not a replacement ROM manager or emulator.

Community and critic scores are imported from existing websites where supported; the project does not need its own public community-rating service. Personal ratings remain separate from imported scores. Metadata acquisition must preserve local overrides. Default metadata provider, credentials, download rights, and exact API scopes remain implementation choices.

## 15. Existing mods and user control

The experience is hybrid: simple defaults with advanced controls. Detection of existing mods is read-only and does not transfer ownership. Adoption requires a preview, explicit authorization, preservation of the current state, verified file membership, and a managed manifest. Files belonging to another manager remain external until a safe, authorized handoff is possible. Unknown installations can be cataloged while deployment stays disabled.

Profiles and activation are separate: selecting a profile is not proof it is deployed. A Vanilla profile disables Game Manager-managed mods; it cannot certify a pristine official installation. The launcher must resolve installation, launch profile, mod profile, and readiness together.

## 16. Original feature vision and future scope

Preserve the original broader vision without expanding the MVP:

- Metadata: title, aliases, release date, developer, publisher, genres, tags, features, source/library, covers, backgrounds, imported community/critic scores, personal score, completion status, and categorized links.
- Personal organization: game notes, journal, to-do objectives, favorites, Now Playing, Play Next, backlog, playtime, activity history, and statistics. Expand these after the core library and safe mod workflows.
- Media and protection: screenshot and recording organization, trailers, background music, and game-save backups are planned expansion areas. Save backup is separate from database backup and deployment-original preservation; its game-path policy needs a later design.
- Interface: one polished dark theme first, light theme later, a personal website-inspired theme, richer original themes, dashboards/widgets, and coherent visual richness later. The personal theme's visual reference still needs to be supplied when designed.
- Fullscreen refers both to the manager's controller-friendly presentation and supported game launch preferences. Start with basic fullscreen later; expand toward trailers, achievements, and music. Do not promise a universal way to force every game into fullscreen.
- Android companion: optional later access to library, stats, and notes; synchronization and authentication require a later specification. Remote game launching is not a confirmed companion requirement.
- Game Tools: Wand-inspired maps, guides, utilities, and cheats/trainers are later scope. Trainer progression is external tool registry/launching, deeper external integrations/presets, and potentially native features for selected games after separate design. No internal trainer implementation or provider has been approved.
- Built-in functionality first. Community extensions and an in-app extension marketplace come much later, after stability, security boundaries, versioning, and a stable extension API.

No Playnite migration is required: the user is willing to rebuild the library. General library export/import and backup remain MVP reliability requirements. Optional cloud synchronization must never become a core dependency.

## 17. Requirement verification retained from the existing package

| ID | Requirement | Future acceptance evidence |
| --- | --- | --- |
| LIB-01 | Persist local library entries across restart | Create/read/update/restart fixtures |
| LIB-02 | Separate canonical, source, entry and installation identities | Idempotent imports and grouping fixtures |
| LIB-03 | Bound Details View data and mounted rows | Instrumented 15,000-game benchmark |
| LIB-04 | Search, sort, filter and retain stable selection | Functional scenarios and latency measurements |
| LIB-05 | Manual entries and overrides survive refresh | Offline editing and refresh checks |
| LIB-06 | Lazy artwork, bounded caches, placeholders | Missing/corrupt art and memory checks |
| LIB-07 | Background work remains observable and cancelable | Concurrent import/navigation cases |
| MOD-01 | Show installation, profile, activation and backend | Plan-review fixture |
| MOD-02 | Preview writes, removals, conflicts and originals | Deterministic plan fixtures |
| MOD-03 | Preserve originals and external changes | Deployment and undeploy failure tests |
| MOD-04 | Validate archives and destination containment | Malicious archive and path fixtures |
| MOD-05 | Journal mutations and recover interruption | Restart and fault injection |
| MOD-06 | Verify before reporting success | Manifest/content/ownership comparisons |
| MOD-07 | State compatibility limits per adapter | Edition/build/loader support matrix |

Keyboard navigation, scaling, loading/empty/error states, and accessible controls must be part of UI design. Exact accessibility conformance and layouts remain design work. No verification in this table has been run.
