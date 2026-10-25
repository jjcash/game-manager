# Game Manager

A free and open-source, local-first Windows desktop game library, launcher, and integrated mod manager in early development. Large-library performance, safe mod operations, and a polished interface are core requirements.

**M0.1 desktop bootstrap is complete.** The application shell lives in `apps/desktop/`; debug and release builds have been launched with real Rust IPC verified. Library and mod functionality are clearly marked placeholders. See [project status](docs/PROJECT_STATUS.md) for verified progress; no performance target has been measured.

The [M0.1 verification report](docs/development/M0.1_VERIFICATION.md) records the environment, exact dependency versions, checks, file inventory, and limitations.

## Read the documentation

| Document                                                        | Purpose                                                       |
| --------------------------------------------------------------- | ------------------------------------------------------------- |
| [AGENTS](AGENTS.md)                                             | Permanent agent instructions, scope and safety                |
| [Project status](docs/PROJECT_STATUS.md)                        | Actual implementation status                                  |
| [Roadmap](docs/ROADMAP.md)                                      | M0–M5 gates and later work                                    |
| [PRD](docs/specifications/PRD.md)                               | Full product baseline and confirmed priorities                |
| [TAS](docs/specifications/TAS.md)                               | Modules, interfaces, workflows and performance budgets        |
| [DSS](docs/specifications/DSS.md)                               | Detailed identity/schema/storage design and reference SQL     |
| [MDRS](docs/specifications/MDRS.md)                             | Profiles, ownership, deployment, journaling and recovery      |
| [ADRs](docs/decisions/README.md)                                | Five stable architecture decision records                     |
| [Sources and open decisions](docs/SOURCE_AND_OPEN_DECISIONS.md) | Recovered evidence, corrections and genuine remaining choices |

## Agreed direction

- Tauri 2, React, TypeScript, Rust and SQLite; internal modular architecture and offline core.
- Windows installed application only; portable distribution dropped.
- Library + Mod Manager MVP with limited tested adapters, manual launching, and configurable installation grouping. No Playnite migration requirement.
- Virtualized Details View first, also a virtualized cover grid; 12,320-game reference, 15,000-game release benchmark, 25,000-game stress test.
- Managed copy deployment first, game-specific and launch-time activation, links later, virtual filesystem later. Detect existing mods read-only; adoption is explicit.
- One polished dark theme first. Game Tools, external-first trainers, Android companion, richer themes, dashboards and extensions are later scope.

Integration priority: Steam → GOG → Epic Games → Ubisoft Connect → EA App → Amazon Games → emulators (RomM, RetroArch, RetroAchievements) → Battle.net → itch.io → standalone executables. Manual entry and standalone launching remain MVP requirements.

Mod targets: Half-Life 2, classic Doom II (not the reboot), Stardew Valley, Minecraft, Cyberpunk 2077, Skyrim, Baldur's Gate 3 and Terraria. First candidates: Half-Life 2, classic Doom II and Stardew Valley. These are intended targets, not working compatibility claims.

## Windows development setup

Install the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/): Microsoft C++ Build Tools with **Desktop development with C++** and a Windows SDK, Microsoft Edge WebView2 Runtime, and stable Rust with the `x86_64-pc-windows-msvc` host. Install Node.js (22.22.2+, 24.15+, or 26+; prefer a current LTS), and pnpm 12.15.0. These versions also satisfy the unit-test environment. The repository does not depend on its checkout path.

```powershell
node --version
pnpm --version
rustc --version
cargo --version
rustup show
rustup component add rustfmt clippy

cd apps/desktop
pnpm install --frozen-lockfile
pnpm desktop:dev
```

Open a new terminal after installing prerequisites. If tools are installed but missing from an older terminal's PATH, this session-only adjustment covers the default locations used on the development machine:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\pnpm\bin;$env:Path"
```

`pnpm desktop:dev` starts Vite on `127.0.0.1:1420` and opens Tauri. The **Desktop connection** panel should show **Rust backend connected · Game Manager 0.1.0**. **Check again** repeats the real IPC call. Browser-only previews cannot call Rust and show an actionable error instead. Navigation selects Library, Mods, or Settings placeholders; it does not operate on game files or persist preferences.

## Development commands

Run these from `apps/desktop/`:

| Command              | Purpose                                                         |
| -------------------- | --------------------------------------------------------------- |
| `pnpm desktop:dev`   | Launch the desktop development app with reload                  |
| `pnpm dev`           | Frontend-only Vite server; Rust IPC unavailable                 |
| `pnpm build`         | Type-check frontend/configuration and build frontend assets     |
| `pnpm desktop:build` | Build the release desktop executable without installer bundling |
| `pnpm preview`       | Preview built frontend assets; Rust IPC unavailable             |
| `pnpm typecheck`     | TypeScript checks, including Vite/Vitest configuration          |
| `pnpm lint`          | ESLint; warnings fail the check                                 |
| `pnpm lint:rust`     | Clippy across Rust targets; warnings fail the check             |
| `pnpm format`        | Apply Prettier and rustfmt                                      |
| `pnpm format:check`  | Check Prettier and rustfmt                                      |
| `pnpm test`          | Frontend IPC and shell unit tests                               |
| `pnpm test:rust`     | Rust serialization contract test                                |

The release executable is `apps/desktop/src-tauri/target/release/game-manager.exe`. This is a development verification artifact, not a portable distribution or installed release. Installer format, signing, updating, and the supported Windows matrix remain M5 work. First builds download Rust dependencies and can take several minutes. pnpm and Cargo lockfiles are included; generated assets and build outputs are ignored.

## Source layout and boundaries

```text
apps/desktop/
  src/                 React/TypeScript presentation and temporary state
    components/        Shell icons and backend-connection panel
    ipc/               Narrow IPC wrapper, runtime validation, shared fixture
  src-tauri/
    src/               Rust entry points and read-only app_get_status command
    capabilities/      Main-window command permission
    icons/             Template desktop icons; final branding deferred
  package.json         Development commands and frontend dependencies
  pnpm-lock.yaml       JavaScript dependency resolution
  src-tauri/Cargo.lock Rust dependency resolution
docs/                  Requirements, accepted decisions, roadmap, actual status
```

Rust owns backend logic. React receives a validated `{ appName, version, backend }` status through Tauri IPC; a shared fixture is checked by both Rust and TypeScript tests. The main local window has permission only for that application command, with no filesystem, shell, opener, database, or remote-origin permissions. Release CSP allows local assets and IPC; development CSP additionally permits Vite reload.

SQLite, benchmark datasets, virtualization, imports, search/filtering, launching games, mod operations, integrations, jobs, cloud/mobile/Game Tools, and installer delivery are not implemented in M0.1. M0 as a whole remains incomplete. Do not begin M0.2 without separate authorization.

## License

FOSS intent is confirmed. A specific open-source license still needs selection; no particular license or badge is implied.
