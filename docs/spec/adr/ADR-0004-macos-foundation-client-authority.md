# ADR-0004: arm64 macOS foundation Client authority and deferred Admin

- Status: ACCEPTED
- Date: 2026-08-21
- Clarified: 2026-08-21 (bootstrap target verification wording and TASK-004-before-TASK-003 owner-context sequencing; accepted authority decision unchanged)
- Partially closes: `OQ-001`
- Defers: `OQ-002`, `OQ-010` Admin enablement

## Context

The current development host is arm64 macOS 26.6.2. V1 needs one concrete local platform for early IPC, filesystem and recovery work, but does not yet need to enable privileged Admin operations or third-party Native Plugin execution.

The installed macOS SDK exposes `getpeereid(2)`, `LOCAL_PEERCRED` and `xucred` for local Unix-domain peer credentials. This is host evidence for an ordinary Client contract, not evidence for Admin elevation or Plugin containment.

## Decision

- arm64 macOS is the initial foundation development and verification platform.
- Linux and Windows remain unsupported for implementation/release claims until separately accepted and tested.
- Client IPC uses a Unix-domain socket inside a Core-created, owner-only directory. The directory is `0700`; the socket is owner-only and is never followed through a caller-controlled symlink.
- Core obtains peer effective UID/GID through `getpeereid`; `LOCAL_PEERCRED` may provide additional audit evidence. The peer effective UID must equal the Library owner's recorded UID.
- `PrincipalContext` is constructed by Core from verified channel evidence. Request fields cannot supply or override actor, role or Admin status.
- TCP and public loopback listeners remain disabled.
- For first creation only, Core may enter a one-shot bootstrap lifecycle before Client IPC: the target must be absent or empty beneath a non-symlink local APFS parent owned by the daemon effective UID; Core creates an owner-only Library root and records that UID. Existing canonical metadata, ownership/mode mismatch, symlink substitution or unsupported filesystem fails closed. This is not an ordinary Client/Admin RPC.
- Deterministic checksummed forward migrations applied by Core during authenticated startup are internal lifecycle work. Manual/destructive migration administration remains an Admin operation and stays disabled.
- All Admin-sensitive operations remain disabled and return `ADMIN_AUTH_UNAVAILABLE` until a later ADR accepts a macOS user-presence/elevation mechanism under `OQ-010`.
- Third-party Native Plugin execution remains disabled until exact sandbox backend/version and hostile conformance evidence close `OQ-002`.
- TASK-004 creates and opens the durable Library owner/lock context before TASK-003 activates Client IPC. TASK-003 receives that context through composition; its framing/proto/IPC crates do not depend on SQLite and never infer owner authority from daemon eUID, environment, CLI or request data.

## Consequences

- This decision is sufficient for macOS local-filesystem work in TASK-004/TASK-005 and, after TASK-004 supplies the opened owner/lock context, ordinary Client work in TASK-003.
- It makes no containment claim against arbitrary software already running with the Library owner's full macOS account authority.
- It does not claim that arm64 macOS has an accepted third-party Plugin sandbox.
- Plugin grants, Credential administration, audit export, manual/destructive migration administration and destructive operations remain unavailable even to the ordinary owner Client.
- Initial bootstrap and automatic forward migration do not grant a reusable Admin PrincipalContext or expose SQLite/CAS authority to the CLI.

## Verification

- Actor-spoof fields are rejected/ignored and never affect CommandRecord or audit attribution.
- A peer with mismatched UID is rejected before command claim or state mutation.
- Socket-parent ownership/mode and symlink substitution are tested on real arm64 macOS.
- Bootstrap tests accept an absent or correctly owned empty target; they reject existing canonical metadata, non-empty content, ownership/mode mismatch, symlink substitution and unsupported filesystems. The effective UID is recorded only after durable root creation.
- Automatic migration tests verify checksum/order/recovery and accept no caller-supplied SQL or migration bytes.
- Ordinary Client access to Admin operations returns `ADMIN_AUTH_UNAVAILABLE` without privileged intent.
- Architecture test proves no TCP listener and no third-party Native Plugin activation.
