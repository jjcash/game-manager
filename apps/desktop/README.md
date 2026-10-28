# Game Manager desktop

The M0.1 Windows desktop shell uses Tauri 2, React, TypeScript, Vite, Tailwind CSS, pnpm, and Rust. See the [root README](../../README.md) for prerequisites, commands, source boundaries, and current scope, and [project status](../../docs/PROJECT_STATUS.md) for verification results.

M1.1 adds **Add game** and **Manage selected game** to the existing Library. Save metadata explicitly; save membership completion or add another named manual membership. Installation records are optional user assertions, with explicit save/remove actions. All game locations are opaque metadata; editing or deleting a record never accesses game files. Each successful mutation invalidates bounded library pages/counts and clears selection/scroll position while retaining filters. Failed saves preserve editor input. A revision conflict requires closing and reopening the editor before reapplying the draft.

Application schema v2 adds ownership, edit revision, installation-state provenance and field overrides without changing summary queries. Existing databases upgrade transactionally before library access. Benchmark-purpose generator-v1 databases intentionally retain schema v1 and reject manual commands. See [M1.1 verification](../../docs/development/M1.1_VERIFICATION.md) and the [native smoke runners](../../benchmarks/README.md#m11-manual-library-verification).

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
