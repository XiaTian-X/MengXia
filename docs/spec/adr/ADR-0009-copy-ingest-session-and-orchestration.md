# ADR-0009: Copy-ingest session and orchestration boundary

- Status: ACCEPTED
- Date: 2026-08-30
- Applies to: `TASK-007`
- Normative detail: `docs/proposals/TASK-007-GATE-PROPOSAL.md` v0.1.4

## Context

TASK-003 deliberately completed a terminal protocol-1.0 handshake and exposed no
product command. TASK-005 owns descriptor-first source access and durable local CAS;
TASK-006 owns the durable command ledger and atomic Asset/event transaction. The
first product slice must compose those completed boundaries without moving identity,
path, SQLite or effect authority into the CLI or protocol layer.

The canonical documents also assigned copied/recreated/cross-volume Blob-root
rebinding to TASK-007. That operation cannot be represented by the immutable 0001
command-result union and would require a distinct authenticated multi-Location
mutation. Performing it implicitly at startup would violate durable command binding
and ordinary-Client/Admin separation.

## Decision

1. TASK-007 adds protocol 1.1 as one authenticated `SINGLE_COMMAND` session while
   retaining the byte-for-byte terminal protocol-1.0 handshake API and behavior.
   There is one request and one terminal response; no multiplexing, TCP or generic
   operation framework is authorized.
2. The only operation is `asset.ingest.v1` copy mode. Principal comes solely from
   the authenticated peer. The caller supplies a durable command UUID and bounded
   semantic values; actor, Project, object IDs, backend and locator are absent or
   reserved.
3. The application owns the versioned canonical request digest and exact
   validate/open/bind/admit/claim/CAS/complete ordering. Physical durability precedes
   atomic graph/event registration. No post-claim response is emitted before a
   durable terminal, recovery or completion outcome exists.
4. The daemon constructs one process-wide ingest service for the opened Library.
   Session, active-binding, execution, storage and store admissions are separate,
   finite and joined. Disconnect, deadline and shutdown are cooperative; after
   durable promotion registration is mandatory or the runtime fails closed.
5. Protocol errors use the exact accepted code/message/retry allowlist. A store
   claim returning `StorageIo`, `StorageCorruption` or `Internal` preserves the
   existing TASK-006 fatal gate: no product response is sent and the current runtime
   begins failed shutdown.
6. Production configuration is resolved once as
   `CLI > environment > explicitly selected external owner-only Library config >
   compiled default`. Store/storage crates accept typed immutable configuration and
   never capture ambient configuration.
7. Same-device/same-inode Blob-root rename retains the accepted backend identity.
   A copied, recreated or cross-volume root fails closed before endpoint publication
   and never rewrites or aliases an existing Location.
8. TASK-008 may verify and report affected custody but may not rebind it. A future
   rebind command requires a separately accepted result/transaction/restart contract
   and resolution of `OQ-010`; no current task owns that mutation.

## Consequences

- TASK-007 depends directly on TASK-003, TASK-005 and TASK-006.
- Core-proto remains transport-only, app remains framework-neutral, storage retains
  path/CAS authority, and store retains durable principal/command/transaction truth.
- Migration 0000 and immutable 0001 remain unchanged. TASK-008 and later behavior
  remain unauthorized.
- Lost responses are retried only with the same caller-retained command ID; the
  server performs no automatic retry.
- A changed storage instance sacrifices ingest availability until an explicitly
  authorized future reconciliation path exists; it never sacrifices custody truth.

## Verification

- AC-001 through AC-009 and every stable TASK-007 TEST ID in the accepted supplement
  are mandatory.
- Protocol compatibility tests prove the retained terminal 1.0 behavior and exact
  descriptor/preflight rules for 1.1.
- E2E, duplicate, saturation, source-race, root-identity, fatal-store,
  cancellation/shutdown and all sixteen KILL boundaries are executable.
- Completion requires `scripts/verify-task-007.sh formal` on the exact committed
  candidate plus reviewed arm64 `macos-26` evidence; developer results alone cannot
  mark the task DONE.
