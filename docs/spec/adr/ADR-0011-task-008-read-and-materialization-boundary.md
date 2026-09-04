# ADR-0011: TASK-008 read, verification and materialization boundary

- Status: ACCEPTED
- Date: 2026-09-04
- Applies to: `TASK-008`
- Normative detail: `docs/proposals/TASK-008-GATE-PROPOSAL.md` v0.2.5

## Context

TASK-007 completed one authenticated copy-ingest path over an immutable migration
0001 command/Asset/event model. TASK-008 must add bounded Asset reads, explicit
Library verification, one-file materialization, Core observability and truthful
health without adding migration 0002, weakening local authority, exposing CAS paths,
or changing the accepted ingest recovery state machine.

The existing schema has no filtered creation-event index, no independently
queryable materialization-result shape, and no durable verification report. Its
`ASSET_REVISION` result kind can represent the selected immutable revision, but the
existing replay mapper assumes the revision-create operation and its DomainEvent.
The canonical health and observability sections also require an executable Core
baseline before later Provider/Plugin work exists.

## Decision

1. Protocol 1.2 adds exactly six owner-Client operations: Library status, normal or
   explicit deep verification, bounded issue listing, bounded Asset listing,
   bounded revision inspection and one-member no-clobber materialization. Protocol
   1.0 and 1.1 behavior remain supported; the exact reviewed 1.1 source, descriptor
   and provenance become immutable test fixtures before the current artifact moves.
2. ListAssets uses an allocator-checked DomainEvent snapshot and scans at most 256
   contiguous commit-sequence rows per page, stopping at page fullness, window end
   or snapshot end. InspectAsset uses hierarchical existing indexes and per-Blob
   `(backend_id, location_id)` Location keysets. The exact cursor formats and all
   work/row/select caps are those in the accepted supplement. No index or migration
   change is allowed.
3. Verification reports are bounded, in-process diagnostic observations. NORMAL
   validates metadata and filesystem authority without hashing Blob bodies; DEEP is
   explicit, single-flight, deadline/cancellation controlled and O(buffer). Neither
   mode mutates canonical rows, lifecycle, `verified_at`, Blobs or orphans.
4. Materialization binds a caller command ID to the exact immutable graph member and
   destination selector. It uses a dedicated observe/new-claim/physical-classify/
   CAS-reacquire/complete/disposition/replay contract. Existing external-ingest
   claim/replay types and ADR-0009 behavior do not change. A completed materialize
   result reuses `ASSET_REVISION` only after dispatch by operation ID and emits no
   DomainEvent.
5. The destination is an opaque descriptor-first authority on local ownership-aware
   APFS. One fixed 512-byte intent is the sole cleanup/recovery authority. Intent and
   staging are synchronized before bounded verified copy; publish is same-directory
   no-replace; the final and parent are synchronized before the command completes.
   No overwrite, directory materialization, cross-device fallback, CAS disclosure or
   automatic orphan deletion is permitted.
6. Endpoint binding and readiness are distinct. Before local command classification
   completes, the authenticated endpoint is status-only and reports `NOT_READY`.
   `READY` requires Library authority, schema validity, an operational writer and a
   bounded durable classification of every prior-runtime claim at the captured
   boundary. Provider degradation cannot relabel a fatal local invariant.
7. Core observability uses the supplement's closed typed log events, bounded
   non-blocking sink, stable per-error counters, finite label registries, duration/
   latency histograms and independent liveness/readiness/availability/security/
   custody fields. Paths, locators, content values, secrets and arbitrary strings are
   excluded from logs, metrics and error details.
8. The new verify and materialize timeout ceilings and all fixed correctness caps in
   the supplement are accepted under CFG-003. Production composition retains
   `CLI > environment > Library config > compiled default`; downstream crates accept
   typed immutable values and never read ambient configuration.

## Consequences

- Migration 0000 and 0001 remain byte-identical; TASK-009 retains migration 0002.
- Materialization history is replayable only from the exact request plus its durable
  CommandRecord; no command-ID-only history query is claimed.
- Verification/custody observations reset on restart and do not claim durable health
  history. A later persisted health model requires its own schema and decision gate.
- TASK-008 supplies only the local-first prerequisite seam for AC-015. Provider Runs
  and final AC-015 PASS remain owned by TASK-015.
- Root rebind, Admin, Provider, Plugin, Credential, Rights, GC, Purge and TASK-009+
  behavior remain unauthorized.

## Verification

- AC-017, AC-018 and AC-019 plus all twenty-one TASK-008 TEST IDs in the accepted
  supplement are mandatory.
- Query-plan tests run against the bundled SQLite 3.53.4 runtime and reject temporary
  sorts, unbounded residual scans and missing endpoint/keyset index use.
- Fault, cancellation, same-OS SIGKILL and real APFS tests cover every accepted
  materialization prefix; power-loss durability is not inferred from SIGKILL.
- Completion requires the developer gate, exact-scope diff/security review, the
  formal `macos-26` aggregate and retained real-second-UID evidence for the exact
  committed candidate. No local-only result can mark TASK-008 done.
