# Game Manager desktop

M1.9 adds **Choose local artwork…**, **Replace local artwork…** and **Clear artwork** for manual canonical games. Native selection accepts one local PNG/JPEG within 8 MiB, 4096 pixels per dimension and 4,194,304 pixels total. Cancellation changes nothing; failed replacement preserves the prior assignment. Details View and the editor display only 64×36 derived images. The external source is never modified and can be removed after successful import.

Application schema v5 adds asset metadata and assignments; exact v2/v3/v4/v5 backups remain supported, with only owned active copies upgraded. Database snapshots include assignments, **never artwork image files**. Keep the configured data root's `database/artwork/originals/` separately. Restore preserves existing originals and shows placeholders for missing files. Originals and uncertain interrupted files have no automatic garbage collection. Derived `database/artwork/thumbnails/` files alone are disposable. See [M1.9 verification](../../docs/development/M1.9_LOCAL_ARTWORK_VERIFICATION.md).

The M0.1 Windows desktop shell uses Tauri 2, React, TypeScript, Vite, Tailwind CSS, pnpm, and Rust. See the [root README](../../README.md) for prerequisites, commands, source boundaries, and current scope, and [project status](../../docs/PROJECT_STATUS.md) for verification results.

M1.1 adds **Add game** and **Manage selected game** to the existing Library. Save metadata explicitly; save membership completion or add another named manual membership. Installation records are optional user assertions, with explicit save/remove actions. All game locations are opaque metadata; editing or deleting a record never accesses game files. Each successful mutation invalidates bounded library pages/counts and clears selection/scroll position while retaining filters. Failed saves preserve editor input. A revision conflict requires closing and reopening the editor before reapplying the draft.

The M1.1 application schema v2 adds ownership, edit revision, installation-state provenance and field overrides without changing summary queries. Existing databases upgrade transactionally before library access. Benchmark-purpose generator-v1 databases intentionally retain schema v1 and reject manual commands. See [M1.1 verification](../../docs/development/M1.1_VERIFICATION.md) and the [native smoke runners](../../benchmarks/README.md#m11-manual-library-verification).

M1.4 adds Settings backup/restore with native file selection, strict snapshot validation, explicit replacement confirmation and restart-bound recovery. The application database path/schema and foreground queries remain unchanged; a process ownership lock prevents concurrent application instances at the same root. See [M1.4 verification](../../docs/development/M1.4_BACKUP_RESTORE_VERIFICATION.md) for current checks and limits. Backups contain database metadata only; game/mod files and artwork image files are excluded, while v5 artwork assignments remain database metadata.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

M1.5 adds schema v3 installation groups scoped to one canonical game, with at most one group per installation. Use the compact grouping section of Manage selected game for explicit create/rename/assign/unassign/removal; duplicate names retain independent IDs. Grouping is user organization only and does not alter presence or provider metadata. Historical migrations and benchmark schema v1 remain unchanged. See [M1.5 verification](../../docs/development/M1.5_INSTALLATION_GROUPING_VERIFICATION.md).

M1.6 adds persistent user-owned LaunchProfiles under [ADR-010](../../docs/decisions/ADR-010-installation-grouping-and-launch-profile-identity.md), with independent identities/revisions, immutable installation bindings and explicit game defaults. Expand LaunchProfiles in the editor to configure standalone tagged references, literal JSON argument arrays and optional working-directory references. Windows lexical validation performs no filesystem access or execution. Installation/membership/game deletion refuses dependent profiles. Application schema v4 accepts exact v2/v3/v4 backups and upgrades only owned active copies. The editor caps total profiles at 100 per game. See [M1.6 verification](../../docs/development/M1.6_LAUNCH_PROFILE_VERIFICATION.md); the explicit M1.8 Launch action executes supported saved profiles.

M1.8 adds **Launch game** to Manage selected game. Choose a supported saved profile or use its explicit saved default, then activate **Launch**. Only user-owned standalone `.exe` profiles on manual Windows installations execute. Selection does not save a default, and nothing launches on startup/edit/restore. Run unelevated; success reports process creation only. See [ADR-011](../../docs/decisions/ADR-011-standalone-launch-execution-contract.md) and [M1.8 verification](../../docs/development/M1.8_PRODUCTION_STANDALONE_LAUNCH_VERIFICATION.md).

The local native approval gate runs both complete targets, with no outer ignored/filtered tests:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --all-features --locked --test standalone_launch -- --test-threads=1 --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --all-features --locked --test standalone_launch_production -- --test-threads=1 --nocapture
```

Require 12/12 and 9/9 respectively in an ordinary unelevated Windows desktop. Hosted CI compiles these targets only; local evidence is a separate manual approval gate. Repeat after launch/security/dependency changes and record the exact tested tree under the accepted policy.
