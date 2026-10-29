# Game Manager desktop

The M0.1 Windows desktop shell uses Tauri 2, React, TypeScript, Vite, Tailwind CSS, pnpm, and Rust. See the [root README](../../README.md) for prerequisites, commands, source boundaries, and current scope, and [project status](../../docs/PROJECT_STATUS.md) for verification results.

M1.1 adds **Add game** and **Manage selected game** to the existing Library. Save metadata explicitly; save membership completion or add another named manual membership. Installation records are optional user assertions, with explicit save/remove actions. All game locations are opaque metadata; editing or deleting a record never accesses game files. Each successful mutation invalidates bounded library pages/counts and clears selection/scroll position while retaining filters. Failed saves preserve editor input. A revision conflict requires closing and reopening the editor before reapplying the draft.

The M1.1 application schema v2 adds ownership, edit revision, installation-state provenance and field overrides without changing summary queries. Existing databases upgrade transactionally before library access. Benchmark-purpose generator-v1 databases intentionally retain schema v1 and reject manual commands. See [M1.1 verification](../../docs/development/M1.1_VERIFICATION.md) and the [native smoke runners](../../benchmarks/README.md#m11-manual-library-verification).

M1.4 adds Settings backup/restore with native file selection, strict snapshot validation, explicit replacement confirmation and restart-bound recovery. The application database path/schema and foreground queries remain unchanged; a process ownership lock prevents concurrent application instances at the same root. See [M1.4 verification](../../docs/development/M1.4_BACKUP_RESTORE_VERIFICATION.md) for current checks and limits. Backups contain database metadata only; game/mod files and artwork are excluded.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

M1.5 adds schema v3 installation groups scoped to one canonical game, with at most one group per installation. Use the compact grouping section of Manage selected game for explicit create/rename/assign/unassign/removal; duplicate names retain independent IDs. Grouping is user organization only and does not alter presence or provider metadata. Exact v2 and v3 backups are supported; a v2 backup upgrades only after copying to the active database. Historical migrations and benchmark schema v1 remain unchanged. [ADR-010](../../docs/decisions/ADR-010-installation-grouping-and-launch-profile-identity.md) defines the future LaunchProfile contract; no production launch profiles or engine are implemented. See [M1.5 verification](../../docs/development/M1.5_INSTALLATION_GROUPING_VERIFICATION.md).
