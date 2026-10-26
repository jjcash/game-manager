# ADR-006: Software License

Status: **Accepted**. Decision date: 2026-10-25, by explicit user instruction.

## Context

Game Manager is intended to be free and open source. Earlier documentation confirmed that intent but left the specific software license unresolved. The user has now selected GNU GPL version 3 or, at the recipient's option, any later version.

## Decision

License original Game Manager software code under **`GPL-3.0-or-later`**. Recipients may use GNU General Public License version 3 or, at their option, any later version published by the Free Software Foundation.

Keep the complete, verbatim [GNU GPL version 3 text](https://www.gnu.org/licenses/gpl-3.0.txt) in the root [LICENSE](../../LICENSE). The README's express later-version grant, package metadata, and first-party SPDX identifiers establish the project's version choice; the standard license text itself remains unmodified.

This decision resolves open choice O-10 in the [decision register](../SOURCE_AND_OPEN_DECISIONS.md). It does not change ADR-001 through ADR-005. The historical chat's preliminary ADR-006 concerned installation grouping; that historical label is unrelated to this repository record.

## Rationale

Preserve recipients' access to source code and their ability to study, modify, and redistribute distributed derivatives under the GPL's terms. The later-version option lets recipients choose a later GPL version published by the Free Software Foundation.

## Consequences

- Distribution must comply with the applicable GPL terms, including retaining required notices, providing the license, identifying modifications as required, and supplying Corresponding Source through a permitted method when conveying object code. Future release packaging must include the required notices and source-distribution arrangements. This maintenance task does not implement or certify that release process. See the [official GPL text](https://www.gnu.org/licenses/gpl-3.0.en.html).
- Third-party dependencies retain their licenses. Compatibility, attribution, source obligations, and redistribution rights must be reviewed before combining or distributing third-party components. Selecting GPL-3.0-or-later is not evidence that every existing or future dependency has been audited.
- The project license applies to original Game Manager software code. It does not automatically relicense third-party game artwork, mods, ROMs, trainers, maps, guides, template assets, or other external content. Managing, linking to, or launching external content does not transfer ownership or grant permission to redistribute it.
- Future extensions and marketplace design must consider whether an extension forms a combined program with Game Manager and what distribution obligations follow. Do not assume either that all external extensions are automatically GPL-covered or that all plugin boundaries permit proprietary distribution. Review the actual integration and applicable licenses; no blanket linking exception or marketplace policy is granted here. See the [GNU GPL FAQ on plugins](https://www.gnu.org/licenses/gpl-faq.en.html#GPLPlugins).
- Use `SPDX-License-Identifier: GPL-3.0-or-later` in appropriate first-party source comments. Preserve existing third-party notices. Do not add project-license headers to generated files, vendor code, dependency licenses, or external assets. See [SPDX guidance](https://spdx.dev/learn/handling-license-info/).

## Copyright attribution

Recommended notice, once the holder and applicable years are confirmed:

```text
Copyright (C) <year or year range> <confirmed copyright holder(s)>
SPDX-License-Identifier: GPL-3.0-or-later
```

This is a proposed format, not an asserted copyright notice. The user's confirmation is still needed for the holder name and applicable years; neither the Git author nor an account name establishes legal ownership. No holder name has been inserted into the software files. The Free Software Foundation notice in LICENSE belongs to the license text and does not identify Game Manager's copyright holder. See the [GNU licensing instructions](https://www.gnu.org/licenses/gpl-howto.en.html).

## Scope

Documentation, licensing metadata, and source-comment notices only. No dependency versions, application behavior, architecture boundaries, release workflow, or M0.2 implementation is changed. The decision is accepted; this is not evidence of release readiness or a completed third-party licensing review.

## License-text verification

The requested GNU HTTPS endpoint repeatedly timed out or reset connections during this task. LICENSE was therefore copied without edits from [GNU Emacs' COPYING file at commit 156ce56863b93a9a6e1c06f95ce41f5698d632a9](https://github.com/emacs-mirror/emacs/blob/156ce56863b93a9a6e1c06f95ce41f5698d632a9/COPYING), and independently matched the GPL text in [GNU Coreutils' COPYING file](https://github.com/coreutils/coreutils/blob/master/COPYING). Both downloaded copies and the repository LICENSE have identical SHA-256 hashes:

```text
3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986
```

The copy includes the preamble, all terms and conditions (sections 0–17), and the complete instructions for applying the license. A direct byte comparison against the requested GNU website download remains unavailable because of the connection failures. The canonical GNU URL remains the project's license-text reference; no license terms or project-specific notices were inserted into LICENSE.

Repository verification on 2026-10-25 passed:

- `pnpm install --frozen-lockfile`: lockfile accepted unchanged.
- `pnpm format:check`: existing Prettier and rustfmt checks.
- Prettier check of this new ADR, `pnpm typecheck`, and `pnpm lint`.
- `pnpm test`: 8 frontend tests in 2 files passed.
- `cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps --locked --format-version 1`: parsed the Rust manifest and confirmed its license identifier.
- Local link validation: 78 Markdown link targets across 18 documents exist; contradictory-license search found no conflicting current selection.
- Manifest comparison: the license fields are the only changed settings; dependency versions, scripts, and lockfiles are unchanged.
- Source comparison: ten authored files differ only by their SPDX comment and separating blank line. Generated permissions, template icons, and dependency licenses were not changed.
- `git diff --check`: passed. Changes remain unstaged on `docs/add-gpl-3-license`; nothing was committed or pushed.

Native compilation and Rust tests were not repeated for metadata and comment-only changes. The direct GNU download and copyright attribution limitations above remain explicit.
