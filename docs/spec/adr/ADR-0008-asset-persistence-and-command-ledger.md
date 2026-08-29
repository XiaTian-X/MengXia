# ADR-0008: Asset persistence and durable command ledger

- Status: `ACCEPTED`
- Date: 2026-08-28
- Decision owners: Project user / TASK-006 gate
- Normative supplement: `docs/proposals/TASK-006-GATE-PROPOSAL.md` v0.2.2

## Context

TASK-004 provides an immutable bootstrap migration engine and one bounded SQLite
writer. TASK-005 provides opaque, durable local Blob custody. TASK-006 must add the
first product schema without weakening either boundary, merging Blob identity with
Asset identity, exposing SQLite/path authority, or inventing recovery evidence for
an effect that occurred outside SQLite.

## Decision

1. `0001_library_assets.sql` is immutable after application. Its exact accepted
   bytes are 12,733 bytes with SHA-256
   `91c76e615fe248abd852860dcd42b32a01f6f024e91ac8387f34069be2435db1`.
   It contains the exact objects and constraints in proposal Appendix A, including
   normalized `asset_revision_parents` and the shared
   `event_commit_sequence` allocator.
2. Asset, AssetRevision, Representation, Resource, Blob and Location retain
   separate typed identities. Blob deduplication never deduplicates an Asset.
3. Domain, application/port DTO and SQLite row types are separate. Inputs are
   typed and bounded before admission; row conversion fails closed as corruption.
4. The store injects the durable Library owner UID and a per-open runtime ID. A
   caller cannot supply principal, event semantics, aggregate reference or event
   sequence.
5. SQLite-only revision and Location commands claim, validate, mutate, append
   events and complete in one writer transaction. Only copy ingest may commit a
   standalone `CLAIMED` row before its external CAS effect.
6. A claim from a prior runtime becomes `RECOVERY_REQUIRED` on exact lookup. The
   system does not infer post-CAS completion and exposes no recovery graph-completion
   API in TASK-006. The unregistered Blob remains an observable TASK-005 orphan.
7. Domain and future SecurityAudit events share one transactional, per-Library,
   monotonic allocator. Domain and provenance events are append-only.
8. TASK-006 adds only `OPERATION_CANCELLED` to the stable error taxonomy. Errors are
   static/redacted and contain no SQL, path, UID, locator, digest or raw input.
9. The exact object-safe `AssetUnitOfWork`, DTO/result/error sets, timestamp/ID
   boundaries and error precedence are those in proposal §§5–10. No product IPC,
   source-path handling, CAS orchestration, deletion, Admin or later capability is
   authorized.

## Consequences

- Runtime startup distinguishes the retained bootstrap-only validator from the
  exact current-prefix validator and applies 0001 only while holding Library
  authority and before worker admission.
- A physical Blob may exist without a registered Asset after an uncertain external
  completion. This is intentionally fail closed and is reconciled by later reviewed
  work, never by guessed state.
- Future schema or recovery changes require a new migration and ADR; applied 0001
  bytes cannot be rewritten.

## Verification

Acceptance is AC-082 through AC-090. The fourteen TEST-*-006 obligations and their
developer/formal aggregate are defined by Specification v1.1.22 and the accepted
supplement. Completion requires the exact checked-in migration digest, migration /
schema corruption matrices, concurrency and replay tests, lifecycle/fault evidence,
retained TASK-001 through TASK-005 gates and reviewed formal CI.
