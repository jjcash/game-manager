# ADR-008: Steam Local Discovery Threat Model B

Status: **Accepted — 2026-10-28** by explicit user approval of the same-user attacker exclusion and hard-link/sensitive-byte limitation below.

Implementation status: **No safe native reader has been implemented or verified.** M1.3 remains incomplete. M1.3C is authorized only after this accepted ADR is merged into `main` and work starts from a clean baseline containing it.

## Context

The [M1.2 contract, section 8](../development/M1.2_STEAM_DISCOVERY_VERIFICATION.md#8-read-permission-and-filesystem-contract-for-a-future-reader) and [M1.3A threat model](../development/M1.3A_WINDOWS_READER_SECURITY.md#threat-model) required stronger object-provenance confinement against deliberate local rename, replacement and link attacks. The [M1.3 security stop](../development/M1.3_STEAM_PREVIEW_VERIFICATION.md) and M1.3A investigation did not establish that guarantee or a production reader. The native probe is limited API evidence, not a safe-reader implementation.

The [M1.3B decision review](../development/M1.3B_STEAM_DISCOVERY_DECISION.md) distinguishes malicious metadata and ordinary concurrent updates from deliberate sensitive-file substitution by another same-user process. Its proposed architectural-decision paragraph is the basis of this accepted decision. The user has explicitly approved model B, including both exclusions below; runtime root selection alone would not constitute that architectural approval.

## Accepted decision

For **credentials-free local Steam installed-app discovery only**, replace M1.2 section 8/M1.3A's strong object-provenance confinement requirement with **threat model B: a restricted local metadata capability bound to a live native root identity**.

Approval authorizes eligible metadata names under the selected native root. It does not certify exclusive in-root provenance of the objects behind those names. Deliberate planting, copying or linking of sensitive objects, or attacks on the consent/open boundary by another same-user process, are outside this capability's confidentiality guarantee. Process injection, a compromised application/OS/kernel/filesystem and privileged attackers are also excluded. This is an explicit product risk choice, not a claim that such attacks are harmless or that the reader sandboxes other same-user applications.

An object accessible through an eligible name is within that namespace authority in principle even if it has another hard-link name elsewhere. **Reject observed multiple hard links before content reads** as conservative hardening. A read-capable open followed by rejection is distinct from an actual content read; it does not meet model A's stronger prohibition on prohibited read-capable opens. Link-count checks do not prove historical provenance or defeat adversarial link-count races. No exclusive in-root provenance guarantee or global hard-link search is required by B.

No credential, session or account stores may be targeted. Absolute exclusion of sensitive bytes deliberately substituted at an eligible metadata name is not promised. Malicious metadata, malformed paths/names, unsupported objects, accidental misconfiguration and ordinary concurrent Steam changes remain in scope. Static reparses and detected replacements must still be rejected; the attacker exclusion does not permit arbitrary path opens or weakened allowlists.

## Retained requirements

The [M1.3B access policy and native verification gates](../development/M1.3B_STEAM_DISCOVERY_DECISION.md#auditable-root-access-design-proposed-under-b) remain design and exit requirements, subject to native implementation validation:

- Explicit user selection and confirmation before metadata content reads, bound in Rust to the acquired native root identity and retained directory handles. A frontend path or boolean is not authority. No startup scan, implicit extra-root grant or silent rebinding after replacement.
- Reviewed native root acquisition and handle-relative access with no-reparse protection at ancestors, root, literal `steamapps` directory and leaves. Refuse unsupported roots, volumes, objects and observed identity changes. Local fixed NTFS is the initial feasibility target, not an established support matrix.
- One-level metadata enumeration and exact ASCII `appmanifest_([1-9][0-9]{0,9})\.acf` leaf allowlisting, with AppID in `1..=4294967295` and filename/body agreement. Reject aliases, malformed paths, ADS and unexpected names. No game-directory traversal or manifest-derived filesystem operations. The optional literal `libraryfolders.vdf` policy does not authorize opening its candidate roots and is outside the initial M1.3C slice.
- Existing regular files only, minimum read/attribute rights, read-only operation and compatible read/write/delete sharing. No creation, mutation, repair, execution, downloads or locks intended to block Steam. Validate observed type, attributes, link count and size before content reads.
- Bounded enumeration, bytes, attempts, retries, elapsed work, workers and retained state. Retain M1.2 parser limits: 64 KiB per manifest, 2 MiB per optional index, 32 roots, depth 16, 4,096-byte tokens and 32,768 pairs. Proposed later session limits remain one active scan, 25,000 records, 128 MiB aggregate input and 100-row pages; these require measurement and do not change M0 policy. Charge retries and repeated observations to budgets.
- Check identity/size/time and two bounded byte observations during ordinary concurrent updates. Permit at most one bounded retry, then warning/incomplete results. Equal observations are not an atomic Steam snapshot or an adversarial ABA proof. Partial/cancelled work establishes no absence, deletion or entitlement.
- Preserve privacy: no raw metadata, account identifiers, unknown values or full paths in logs/events; selected-path context belongs only in local consent/review. No authentication, account access, credentials, networking or executable reads. Installed-app evidence is not complete ownership or launchability evidence.
- Rust-owned resources, cancellation and verified handle cleanup on success and all failure paths. A timeout does not terminate native I/O. Do not report cleanup complete until outstanding operations finish and resources are released. Future production access requires narrow typed IPC and opaque scoped grants; this ADR adds no IPC.

## Narrow supersession

| Earlier requirement                                                                                                                       | Effect of this ADR                                                                                                                                                                                                       |
| ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| M1.2 section 8, especially items 4–5: refuse hard-linked metadata whose provenance cannot be confined                                     | Superseded only as an exclusive-provenance guarantee for credentials-free local Steam installed-app discovery. Native identity, no-reparse access, exact names and conservative observed-link rejection remain required. |
| M1.2 section 8 item 8: never read credential/session/account data                                                                         | Intentional targeting remains prohibited. Only the absolute guarantee against sensitive bytes deliberately substituted at an eligible name by another same-user process is superseded in this scope.                     |
| M1.3A threat model: no content outside the approved native directory identity opened for reading, including deliberate local substitution | Superseded only in this scope by B's namespace authority and explicit confidentiality exclusions. Model A has not been demonstrated.                                                                                     |

The earlier M1.2, M1.3, M1.3A and M1.3B investigation reports remain unchanged historical evidence. Their task-time stops, proposed status and stronger wording must be read with this later accepted decision; their test results are not upgraded. No unrelated security requirement, mod ownership/deployment rule, other integration policy, manual-library architecture or accepted [ADR-007 M0 policy](ADR-007-conditional-m0-performance-gate.md) is superseded.

## M1.3C authorization and merge gate

The user separately authorizes **M1.3C — Isolated Windows Steam Metadata Reader Feasibility**, conditional on this ADR being merged into `main` and a clean baseline containing it. Until then, prepare and verify documentation only, then stop. Do not commit, push, merge or begin implementation automatically.

After that gate, the authorized slice is a small isolated Rust candidate over **one explicitly selected disposable local NTFS root**, literal `steamapps` and exact manifest leaves. Establish native acquisition/identity, no-reparse access, conservative validation/link rejection, bounded existing-file reads, ordinary-update consistency, cancellation and cleanup. Integrate the existing M1.2 parser only after native access tests pass.

Use disposable junction/symlink fixtures where available, root replacement, hard links, locked and unlocked external sentinels, malformed paths, concurrent changes, limits and cancellation. Instrument attempted opens separately from actual content reads. Record actual Windows/filesystem scope, untested cases, handles/workers/memory, quality checks and maintenance cost. Unavailable coverage is a gap, not a pass; an inconclusive investigation is not a successful reader and cannot claim model A.

No Steam discovery UI or production IPC, persistent grants, real Steam scanning, account access, game-directory traversal, migration, importing/catalog writes or other storefront work is authorized. Keep the manual-library architecture and M0 performance policy intact. No commit or push is authorized.

Only after undertaking M1.3C, create `docs/development/M1.3C_WINDOWS_READER_FEASIBILITY.md` with implementation details, native test evidence, resource measurements, checks, files changed, limitations and cost-benefit assessment. Conclude with exactly one recommendation:

- **Resume M1.3:** isolated reader satisfies model B within the supported scope and is maintainable.
- **Defer Steam local discovery:** implementation remains unsafe, insufficiently verified or disproportionately expensive.

M1.3 remains incomplete regardless of M1.3C's result. Report and stop; later milestones do not begin automatically.

## Consequences

Model B makes a capped native feasibility investigation proportionate to installed-app discovery without requiring a bespoke same-user filesystem sandbox. It does not resolve acquisition/API semantics, establish Windows support, prove confidentiality against substitution or guarantee that implementation cost is justified. Failure or insufficient verification means deferral. Manual offline use remains available and the established integration priorities stay intact.
