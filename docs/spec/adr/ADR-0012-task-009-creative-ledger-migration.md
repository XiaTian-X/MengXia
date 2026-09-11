# ADR-0012: TASK-009 creative model and extensible ledger migration

- Status: `ACCEPTED`
- Date: 2026-09-09
- Applies to: `TASK-009`
- Normative candidate: `docs/proposals/TASK-009-GATE-PROPOSAL.md` v0.1.5

## Context

The immutable TASK-006 migration permits only the Asset-era CommandRecord result
and DomainEvent aggregate kinds. TASK-009 needs replayable Project, Subject, Work
and Take outcomes and events. SQLite cannot widen the existing CHECK constraints in
place, and editing migration 0001 would invalidate every existing Library.

The creative model also needs immutable ProjectSpecRevision/WorkRevision records,
optimistic concurrency, explicit Take transitions, global Asset/Subject references
without Project ownership, bounded JSON and stable query pagination. TASK-009 must
provide these without implementing Run/Provider generation, Admin trust changes or
destructive storage behavior.

## Decision

1. Migration 0002 performs one reviewed rebuild of `commands` and `domain_events`,
   preserves every old shared-column byte and adds bounded versioned payloads plus
   syntactically extensible result/aggregate tokens. The payload is authoritative for
   new results; their UUID `result_id` projection is optional at the schema layer,
   while all TASK-009 UUID-primary codecs require it and the three legacy kinds keep
   their exact non-null/payload-free rules. The running binary retains a closed
   operation/event/result codec registry; unknown or inconsistent values fail as
   corruption. Blob-digest, no-primary-object and composite future results therefore
   do not require another ledger-table rebuild.
2. The exact SQL candidate and digest are fixed by the TASK-009 proposal. Migration
   0000 and 0001 remain byte-immutable. Old Asset, AssetRevision, Location and
   materialization outcomes/events must replay identically after upgrade.
3. Before the destructive rebuild, the exclusively opened daemon creates a fixed,
   durable, owner-only, verified exact-0001 snapshot under a checksummed immutable
   intent that remains its restore manifest for the snapshot's full lifetime. Crash
   recovery recognizes only the proposal's finite link-count/journal state matrix.
   Snapshot validation uses the exact no-sidecar immutable read-only URI contract;
   ordinary read-only open is forbidden. The checked-u128 capacity formula consumes
   the same resolved Library reserve as BlobStorage. The final snapshot/manifest pair
   is retained for explicit offline restore; this task adds no automatic restore or
   deletion authority. Before a previous binary may open the restored Library, every
   TASK-009 migration artifact is moved out of the Library root so that the previous
   binary's exact namespace remains valid.
4. Project is a work/policy context, never a tenant or owner of global Asset/Subject
   identity. New Projects have effective trust `UNTRUSTED`; TASK-009 cannot escalate
   trust and 0002 stores no competing trust column because migration 0004/TASK-013
   owns future authoritative `project_trust` records.
   ProjectSpecRevision and WorkRevision are immutable, while their parent pointers
   advance under optimistic revision checks.
5. Subject identities are Library-global. Each immutable WorkRevision owns its
   complete closed Subject/Asset relationship set; revisions in multiple Projects
   may reference the same global identity. A scoped command must still validate its
   complete Project ancestry. This makes a later Run's WorkRevision binding
   sufficient to recover the selected creative inputs.
6. Take transitions are closed and expected-revision protected. Terminal Takes are
   immutable; reopening creates a new candidate and typed relationship. Replacing a
   selected Take is explicit and atomic. Each Take has at most one outgoing reopen
   and one outgoing supersede edge, enforced by partial unique indexes; bounded list
   results return only those outgoing edges and never collect unbounded incoming
   relationships.
7. Project policy and Work specification JSON are bounded, duplicate-key rejecting,
   canonicalized and treated as opaque data that grants no authority. Exact limits,
   the closed `i64/u64/f64` visitor-level numeric envelope, request digests, wire
   codecs, pagination and operation contracts are those in the reviewed proposal.
8. Protocol 1.3 is additive and retains immutable 1.0/1.1/1.2 compatibility. The
   permanently reserved CoreRequest 8..15 and CoreResponse 8..14 ranges remain
   reserved, response 15 remains ErrorEnvelope, and the fifteen operations use
   request/response tags 16..30. Existing TASK-008 ListAssets/InspectAsset cursors
   advance to exact-0002-bound format 2 and reject format 1; the four new cursor
   families are separately bound to exact 0002. All operations remain authenticated
   ordinary-owner Client operations; no generic CRUD, Admin, Provider, CAS-write or
   later-task surface is added.
9. TASK-009 supplies unscored domain/persistence prerequisite evidence for
   regeneration but receives no AC-010/REQ-003 PASS or CONTRIBUTOR_PASS status;
   ordinary CreateTake may reuse an existing Asset. Final regeneration and
   Run-binding evidence remains with the later runtime/Provider owners. Likewise it
   contributes immutable Run-input foundations and the state/event portion of
   AC-016, while
   TASK-015 retains the not-yet-existing Attempt-history clause.
10. Until TASK-013 introduces the separately governed SecurityAuditEvent stream,
    Take approval and reopen use the proposal's immutable CommandRecord plus ordered
    DomainEvent as durable domain action history. This does not claim SEC-019 audit
    capability; TASK-013 may cross-link but cannot rewrite that history.

## Consequences

- Migration/recovery implementation is larger than an additive-table-only change,
  but there remains one Library-wide command table and one ordered DomainEvent
  stream instead of permanent legacy/v2 parallel authorities.
- The snapshot adds one bounded owner-only recovery artifact. Its removal remains
  unavailable until a later retention/Admin contract explicitly authorizes it.
- New JSON dependency and protocol fields require exact supply/descriptor evidence.
- Stock SQLite's fixed immutable snapshot URI open is the sole path-based exception
  inside an otherwise descriptor-relative snapshot namespace workflow; it remains
  enclosed by whole-prefix and fixed-inode pre/post proofs and does not authorize a
  custom VFS or new FFI.
- TASK-008 readiness, observability, fatal-store behavior and prior replay semantics
  remain mandatory regression gates.
- This ADR grants only the synchronized `TASK_009_ONLY` implementation authority;
  it grants no TASK-010+, Admin, Provider/Plugin, root-rebind or destructive scope.

## Pre-acceptance evidence

- the reviewed proposal, SQL candidate and this ADR agree on the exact migration,
  result/event, snapshot, JSON, protocol, authority and resource contracts;
- the candidate SQL byte count and SHA-256 are independently recomputed, and a
  populated 0001 upgrade executes under the bundled SQLite 3.53.4 with
  `integrity_check=ok`, no foreign-key violation and byte-exact legacy result/event
  preservation;
- the retained TASK-001 through TASK-008 developer baseline and canonical document
  gates pass before TASK-009 is activated.

## Verification required before completion

- full corruption, foreign-key, snapshot, fault and SIGKILL matrices under bundled
  SQLite 3.53.4;
- exact old/new result and event codec golden vectors and replay checks;
- Project/Subject/Work/Take state, concurrency, cross-Project and pagination tests;
- JSON bounds/canonicalization and dependency security/license/MSRV review;
- protocol 1.0/1.1/1.2 compatibility plus exact protocol-1.3 descriptor evidence;
- complete retained repository developer/formal and reviewed `macos-26` evidence.
