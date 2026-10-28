# M1.2 Steam contract fixtures

All files in this directory are authored synthetic fixtures under the repository's GPL-3.0-or-later license. No upstream code, real Steam capture, account identifier, credentials, cookies, entitlement ticket, artwork or game file is included. `fictional-A/B` are model labels, **not SteamIDs**. AppIDs, names, times, roots and the zero `LastOwner` value are invented test inputs. The paths are strings and are never opened.

Run from `apps/desktop`:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked --test steam_discovery
```

| Fixture                     | Purpose                                                                                                                                         |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `installed.acf`             | FullyInstalled manifest, Unicode/quoted/punctuation title, optional unknown nested metadata                                                     |
| `no-cached-name.acf`        | Installed/update-required record with no display-name cache; explicit fallback name                                                             |
| `incomplete.acf`            | Downloading without FullyInstalled; installation remains unknown                                                                                |
| `truncated.acf`             | Incomplete text; per-file warning without losing other valid records                                                                            |
| `libraryfolders-modern.vdf` | Multiple roots, `apps` hints and duplicate lexical root                                                                                         |
| `libraryfolders-legacy.vdf` | Older string-valued roots, case-insensitive known key and Unicode path                                                                          |
| `libraryfolders-unsafe.vdf` | One valid candidate plus UNC/device/relative/traversal/stream attacks                                                                           |
| `path-cases.json`           | Component/root/metadata allowlist matrix, reserved names, NUL and separators                                                                    |
| `policy-cases.json`         | Cached-but-not-installed candidate, duplicates, missing/incomplete observations, ambiguous/selected accounts and family/account-switch contexts |
| `snapshot-cases.json`       | Reader-supplied file identity/time/length/regular/reparse proofs, replacement/in-place change/partial read and size rejection                   |
| `binary-headers.json`       | Invented header prefixes for known/deferred and future/unsupported appinfo/packageinfo versions; malicious offset bytes are never interpreted   |

Additional small inputs generated in Rust exercise file/token/depth/pair/root limits, malformed IDs, unknown flags, unsafe installation components, duplicate keys, unsupported directives and cancellation.

The harness lives in `tests/support/steam_contract.rs` and is imported by one integration test. It is not a production adapter, is not part of `lib.rs`, and has no filesystem, registry, network, Tauri or database API. Source syntax tests are separate from inference tests. Recognizing a binary header is **not** support for parsing its body. A numeric cache directory is only an AppID hint: current library visibility, ownership, sharing and an uninstalled state are not established.

Snapshot/path proof tests model a future reader contract. They do not implement or certify Windows handle-relative access, reparse/TOCTOU protection, a live Steam client or an atomic Steam snapshot. Those are explicit M1.3 validation prerequisites. See [M1.2 evidence and contract](../../../../../../docs/development/M1.2_STEAM_DISCOVERY_VERIFICATION.md).
