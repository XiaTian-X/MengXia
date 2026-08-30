---
title: "TASK-006 Asset domain and persistence start-gate proposal"
project: "梦夏 / MengXia"
document_role: "Accepted TASK-006 implementation supplement"
status: "ACCEPTED_INCORPORATED_BY_CANONICAL_SPECIFICATION_1_1_22"
version: "0.2.2"
date: "2026-08-28"
canonical_specification_reviewed: "IMPLEMENTATION_SPEC.md v1.1.21"
---

# TASK-006 Gate Proposal

## 0. Gate verdict

`TASK-006` is complete. Candidate v0.2.1 passed the requested post-correction
feasibility, safety and downstream-compatibility review, and the user explicitly
authorized execution on 2026-08-28 after the final confirmation. Canonical
Specification v1.1.22 and the exact Plan start record incorporated this supplement;
the exact implementation and review correction then passed the formal completion
evidence recorded in §19. `TASK-004` through `TASK-006` remain `DONE`, and current
implementation authority is `NONE`.

TASK006_CANONICAL_GATE: ACCEPTED
TASK006_LIFECYCLE: DONE
TASK006_IMPLEMENTATION_AUTHORITY: NONE
TASK006_PROPOSAL_VERSION: 0.2.2

No statement in this document authorizes `TASK-007`, a product IPC operation, source
ingest orchestration, CLI behavior, Admin authority, deletion, GC, Provider, Plugin,
Credential, Rights or later migration work.

### 0.1 v0.2.1 external-review correction disposition

| Finding | Disposition |
|---|---|
| source selection absent from ingest digest | exact accepted-path selector digest added in §2 GAP-006-005 |
| pure SQLite command split across transactions | external/pure families separated in GAP-006-008 and §7 |
| recovered completion lacks durable proof | automatic completion/query removed; fail-closed runtime-tagged recovery in GAP-006-009 |
| public port under-specified | exact signatures, owned DTO/result/error sets and derived events frozen in §6.1 |
| one DurableBlob versus multiple Members | TASK-006 production registration narrowed to one Member in §5.3 |
| one timestamp spans claim and verification | claim/completion/pure boundaries separated in §6.2 |
| full FK check described as bounded startup | migration/startup/reader/deep ownership separated in GAP-006-010 and §8.5 |
| SQL/text-validator drift | SQL now rejects multiple media slashes; mapper-only control policy made explicit in §5.4 |
| locator alias error ambiguous | local-CAS cross-digest alias fixed to `STORAGE_CORRUPTION` in §§9–10 |
| terminal rejection cannot be returned by claim | `ExternalClaimOutcome::TerminalRejected` added and bound to §7 precedence |
| successful registration result loses original Location | immutable `commands.result_location_id` added to the frozen result reference |
| unresolved current-runtime claim can remain live | exact synchronous fail-current-runtime port plus app-owned armed guard added in §6.1 |
| TASK-005 stop/entropy outcomes lack a durable mapping | deadline, cancellation and pre-staging entropy mappings frozen in §6.1 and §10 |
| public boundary still leaves IDs/fields to implementation | ownership, constructors, exact registration fields and private app seams frozen in §6.1–6.2 |

The corrections in this table passed the follow-up review and local evidence below.
That review makes the candidate ready for user acceptance; it does not grant
implementation authority or silently synchronize canonical documents.

## 1. Inputs read and prerequisite evidence

The gate is based on the following sources, in authority order:

1. `docs/spec/IMPLEMENTATION_SPEC.md` v1.1.21;
2. `docs/spec/DECISIONS.md` v0.3.21 and ADR-0001 through ADR-0007;
3. `docs/spec/IMPLEMENTATION_REVIEW.md` v1.1.31;
4. `docs/spec/IMPLEMENTATION_PLAN.md` v0.3.31;
5. `docs/spec/PROJECT_INTAKE_REPORT.md` v1.3.26;
6. completed TASK-004 and TASK-005 repository behavior and tests;
7. SQLite 3.53.4 behavior under the pinned bundled runtime.

Prerequisite disposition:

| Prerequisite | Evidence | Result |
|---|---|---|
| TASK-004 complete | commit/run recorded by canonical completion record | PASS |
| TASK-005 complete | commits `88e7b341...`, `f516faaf...`; reviewed run `33073580258` | PASS |
| Current authority | AGENTS/Spec/Plan/Review/Intake all say `NONE` | PASS / implementation remains forbidden |
| Existing product migration | only `0000_store_bootstrap.sql` exists | EXPECTED_GAP owned by TASK-006 |
| Domain/app/event behavior | domain contains only foundation errors; app/events are empty boundaries | EXPECTED_GAP owned by TASK-006 |
| Store product command API | writer/read jobs are private verification-only jobs | EXPECTED_GAP owned by TASK-006 |
| Retained baseline | `document_traceability` 2/2 and `git diff --check` pass before this draft | PASS |

The local branch is one clean user-authored documentation commit ahead of
`origin/main` (`b85c6bb docs: reconcile post-task-005 planning state`). This draft
preserves it and does not push or rewrite history.

## 2. Blocking discrepancies resolved by this proposal

### GAP-006-001 — product-schema validation cannot reuse bootstrap recovery validation

Classification: `CONFLICT / ARCHITECTURE`

The completed store currently treats any object beyond the two bootstrap tables and
their autoindexes as corruption. TASK-006 must add product objects without weakening
TASK-004 crash recovery. The implementation therefore MUST keep two distinct
validators:

- `verify_bootstrap_schema_*`: accepts exactly migration 0000 and is used only for
  bootstrap staging/intent/recovery;
- `verify_current_library_schema_*`: accepts the exact compiled migration prefix for
  an already-published canonical Library and is used before runtime admission.

Bootstrap staging MUST never apply migration 0001. Product migrations apply only to
the already-published canonical database while the durable Library lock is held and
before writer/read workers or product mutation admission start.

The runtime sequence is exact: open one hardened read-write connection, validate an
exact supported prefix, apply/validate 0001 when required, then open hardened
read-only connections and finally spawn workers. Runtime verification jobs switch to
the current-prefix validator; retained bootstrap/recovery fixtures continue calling
the 0000-only validator directly. Updating an assertion from one migration row to
the exact current prefix is not permission to weaken any TASK-004 crash, schema or
hardening test.

### GAP-006-002 — migration registry lacks required normalized state

Classification: `SPEC_STALE / DATA_MODEL`

The canonical 0001 table summary omits:

- `event_commit_sequence`, required so future DomainEvent and SecurityAuditEvent
  writers share one per-Library sequence source; and
- `asset_revision_parents`, required to represent immutable multi-parent revision
  lineage without JSON or a mutable single-parent shortcut.

The accepted canonical synchronization MUST add those two tables to the 0001 row.
They are not a new feature: they close `DATA-010`, `DATA-011` and the proposed
`AssetRevision.parent_revision_ids` shape.

### GAP-006-003 — proposed domain shapes contain unbounded/undefined strings and JSON

Classification: `PARTIALLY_SPECIFIED / SECURITY`

TASK-006 MUST NOT invent or persist arbitrary technical metadata, source paths,
Rights payloads or unbounded JSON. The first migration stores only the typed fields
defined here. Rich technical metadata and generalized provenance assertions require
a later versioned schema and migration under `DATA-012`.

The accepted finite TASK-006 metadata boundaries are:

| Value | UTF-8 byte range | Additional rule |
|---|---:|---|
| domain token (`AssetKind`, `ContentKind`, purpose, resource kind) | 1..64 | lowercase ASCII token grammar `[a-z][a-z0-9._-]*` |
| member logical name | 0..255 | no NUL/control character; not a filesystem path |
| media type | absent or 3..255 | lowercase ASCII `type/subtype` token; TASK-006 registration may use absent |
| Location backend ID | 1..255 | opaque to domain; no NUL/control character |
| Location locator | 1..1024 | opaque to domain; no NUL/control character |
| operation ID | 1..128 | lowercase ASCII dotted token ending in `.v1` |

These are safety/data-format caps, not performance SLOs. The exact TASK-005 local
descriptor remains 85 bytes for both backend ID and locator and passes without being
parsed as a filesystem path.

### GAP-006-004 — Rust unsigned revision and SQLite signed INTEGER do not match

Classification: `DATA_MODEL`

Every persisted `RevisionNo` MUST use an exact eight-byte big-endian BLOB. Row
mappers reject any other length and parse through `RevisionNo`. This preserves the
full accepted `u64` contract and lexicographic byte identity. Sequence numbers and
event commit sequence use SQLite INTEGER only where this proposal explicitly limits
the domain to a checked signed range.

### GAP-006-005 — CommandRecord principal and request digest ownership are unclear

Classification: `ARCHITECTURE / SECURITY`

TASK-006 persists and enforces the durable command ledger but does not accept a
caller-supplied principal. The store binds each new command and every persisted
`created_by_uid` to the owner UID already held in `OpenedLibraryMetadata`. TASK-007
later computes the operation-specific canonical request digest after authenticated
request validation and supplies only that digest plus a typed command ID. Transport
request ID, generated result IDs, sampled timestamps and principal are excluded.

For `asset.ingest.v1`, the digest preimage MUST include the copy-mode tag, every
validated semantic request field and a `source_selector_digest`. After the
descriptor-first TASK-005 opener accepts the normalized absolute source path bytes
(1..1023 bytes), TASK-007 computes exactly:

```text
SHA-256(
  ASCII "MENGXIA_SOURCE_SELECTOR_V1" || 0x00 ||
  u16_be(source_path_byte_length) || source_path_bytes
)
```

Only those 32 selector bytes enter the versioned, tag-and-length-delimited canonical
request serialization; path bytes are never stored, logged, returned or placed in a
domain/event DTO. Optional expected content digest uses an explicit presence byte
followed by its 32 bytes. Thus two selected paths cannot alias absent a SHA-256
collision even when `expected_digest` is absent; a caller intentionally reusing the
same command ID and selector requests replay of the original logical command. A new
selection or changed semantic request requires a different request digest and
therefore conflicts under the old command ID. TASK-007 freezes the complete request
field tags/order and golden vectors before its implementation; it may not omit this
exact selector contribution or compute it before the source path is accepted.

Thus TASK-006 supplies persistence/concurrency evidence for AC-005 through AC-008,
while TASK-007 remains their terminal end-to-end owner, including a real
channel-derived principal mismatch test.

### GAP-006-006 — shared event ordering has no allocator

Classification: `DATA_MODEL / RELIABILITY`

`event_commit_sequence(singleton=1,last_sequence)` is the sole allocator. A writer
transaction increments it with checked signed overflow and uses the returned value
for every DomainEvent in that commit. A transaction that writes multiple events
reserves a contiguous range in one checked update. Rollback rolls back allocation.
Future SecurityAuditEvent persistence MUST consume the same allocator rather than
creating a second sequence.

### GAP-006-007 — TASK-006 acceptance ownership is too broad

Classification: `SPEC_STALE / TRACEABILITY`

The current Plan lists AC-002 and AC-005 through AC-008 directly for TASK-006, even
though their published wording includes ingest commands and authenticated
principals. Canonical synchronization MUST record TASK-006 as a contributor and
TASK-007 as terminal owner for those five ACs. TASK-006 receives the new task-local
AC-082 through AC-090 registry below so it can be completed without falsely claiming
the copy-ingest E2E surface.

`REQ-004` is consumed here only for the generic immutable AssetRevision persistence
and optimistic-concurrency invariant. `TASK-009` remains the terminal owner of
`AC-011` and of editing a selected Project/Work/Take creative object; TASK-006 MUST
NOT add those aggregates, their product workflow or their transport surface.

### GAP-006-008 — external-effect and SQLite-only command transactions differ

Classification: `CONFLICT / RELIABILITY`

Only `asset.ingest.v1`, whose CAS effect lies between intent and registration, may
commit a standalone `CLAIMED` row. `asset.revision.create.v1` and
`blob.location.record.v1` are SQLite-only: their one writer transaction performs
binding lookup/fresh claim, expected-revision validation, state mutation, event
append and terminal outcome. A pure operation never exposes a committed `CLAIMED`
state. This is the TASK-006 interpretation of canonical `DATA-009` and
`REVIEW-006`.

### GAP-006-009 — automatic post-CAS completion lacks durable proof

Classification: `CONFLICT / RECOVERY`

TASK-006 deliberately does not persist a source selector, raw path or an unreviewed
effect-observation payload. It therefore MUST NOT automatically complete a crashed
post-CAS registration. A committed external claim is tagged with the creating store
runtime ID. On a later-runtime exact retry it atomically becomes
`RECOVERY_REQUIRED`; the returned safe result directs the operator to orphan
reporting/reconciliation. The physical Blob remains a safe TASK-005 orphan and no
Asset points to it. There is no recovery-only graph completion API. A later automatic
reconciliation design requires a new migration/ADR and cannot be inferred here.

### GAP-006-010 — startup integrity checks need explicit cost ownership

Classification: `PARTIALLY_SPECIFIED / RELIABILITY`

TASK-004 `quick_check` remains exactly once per Library open on the migration-capable
writer and may scale with metadata database size; this proposal makes no constant-
time startup claim. Full `foreign_key_check` runs once immediately after applying
0001 while all new product tables are empty, and in explicit TASK-006 corruption
fixtures. It does not run on every normal reopen or reader connection. Reader
connections validate only the connection-local hardening tuple plus exact Library
identity/current migration prefix. TASK-008 owns explicit deep whole-row FK/domain
integrity traversal and its progress/reporting policy.

## 3. Exact authorized scope after acceptance

Only the following paths may change during implementation. New files must stay
inside the listed directories and serve only the named TASK-006 contract.

```text
.github/workflows/ci.yml                    # rename/execute TASK-006 aggregate only
AGENTS.md
docs/proposals/TASK-006-GATE-PROPOSAL.md
docs/spec/IMPLEMENTATION_SPEC.md
docs/spec/DECISIONS.md
docs/spec/IMPLEMENTATION_REVIEW.md
docs/spec/IMPLEMENTATION_PLAN.md
docs/spec/PROJECT_INTAKE_REPORT.md
docs/spec/adr/ADR-0008-asset-persistence-and-command-ledger.md
migrations/sqlite/0001_library_assets.sql
crates/mengxia-domain/src/**
crates/mengxia-domain/tests/**
crates/mengxia-types/src/error.rs             # additive OPERATION_CANCELLED taxonomy row only
crates/mengxia-types/tests/**                  # exact additive parser/mapping compatibility only
crates/mengxia-events/src/**
crates/mengxia-events/tests/**
crates/mengxia-ports/src/**
crates/mengxia-ports/tests/**
crates/mengxia-app/src/**
crates/mengxia-app/tests/**
crates/mengxia-store-sqlite/src/**
crates/mengxia-store-sqlite/tests/**
crates/mengxia-testkit/tests/document_traceability.rs
crates/mengxia-testkit/tests/task_006_foundation.rs
crates/mengxia-testkit/tests/naming.rs       # only if the new canonical files require inventory sync
scripts/verify-task-006.sh
```

Expected production files are narrower than the path globs: asset domain/event
modules, one app service, one port module, one store repository module, and the
minimum migration/validator/lifecycle integration needed to apply and use 0001.

### 3.1 Explicitly forbidden

- no edit to `proto/**`, `bins/**`, `mengxia-core-proto`, framing or IPC behavior;
- no source path opening, streaming, hashing, CAS mutation or TASK-005 contract edit;
- no `IngestAssetCopy` product endpoint, CLI command or daemon dispatch;
- no arbitrary SQL/Connection exposure and no app/domain dependency on rusqlite;
- no raw caller actor, owner UID, SQLite row or local locator interpretation in domain;
- no Asset retirement/restore, Location removal, GC/Purge or destructive mutation;
- no Project/Work/Take, Plugin, Provider, Credential, Rights or SecurityAuditEvent;
- no arbitrary JSON/metadata persistence, Serde derive or schema-less payload;
- no Cargo manifest/lockfile change and no new dependency or feature;
- no change to any accepted TASK-002 value behavior except the additive,
  non-rebinding `OPERATION_CANCELLED` ErrorCode required by TASK-005 cancellation;
- no new unsafe/FFI or platform authority expansion;
- no rewrite of `0000_store_bootstrap.sql` or any completed migration byte;
- no unbounded list/query, detached worker, automatic retry or background task;
- no TASK-007 or later implementation.

## 4. Requirements and decisions consumed

Feature contribution: `FUNC-002`, `FUNC-003` (domain/persistence precondition only).

Normative requirements:

```text
REQ-001 REQ-002 REQ-004 REQ-005 REQ-008 REQ-011 REQ-012
DATA-001 DATA-007 DATA-009 DATA-010 DATA-011 DATA-013
SEC-017 SEC-020 SEC-021
REL-001 REL-004 REL-005 REL-006
BASE-007 BASE-008 BASE-009 BASE-011 BASE-013 BASE-014 BASE-016 BASE-017 BASE-018
DEC-003 DEC-006 DEC-007 DEC-008 DEC-016 DEC-017 DEC-018 DEC-019 DEC-020 DEC-021 DEC-022
ADR-0001 ADR-0002 ADR-0003 ADR-0004 ADR-0005 ADR-0006 ADR-0007
```

`DATA-002`, `DATA-003` and `DATA-004` are consumed only through the already verified
opaque `DurableBlob` result. TASK-006 does not claim the physical custody step.
`API-010` remains a TASK-007 product-operation gate; this task exposes no transport.

No open OQ blocks this exact scope. OQ-004/OQ-005/OQ-008/OQ-009/OQ-010 and later
OQ-006 portions remain later blockers and their capabilities stay absent.

## 5. Domain contract

### 5.1 Identity and aggregate separation

Public marker types are distinct and have no implicit conversions:

```rust
pub enum Command {}
pub enum Asset {}
pub enum AssetRevision {}
pub enum Representation {}
pub enum Resource {}
pub enum Location {}
pub enum ProvenanceEvent {}
pub enum DomainEvent {}
```

The domain exposes opaque structs with private fields and read-only accessors. It
does not expose SQLite rows or constructors that can create an invalid aggregate.

`Asset`, `AssetRevision` and `Blob` remain different identities. A second Asset may
reference an existing Blob digest. Blob equality never reuses an Asset or revision
ID. Representation/Resource/Member identity is separate from Blob identity.

### 5.2 States

TASK-006 fixes only initial and non-destructive states:

```text
AssetLifecycle: ACTIVE
RevisionCustody: MANAGED
BlobLifecycle: AVAILABLE
LocationLifecycle: AVAILABLE
LocationCustody: MANAGED
LocationDurability: DURABLE
ProvenanceVerification: VERIFIED
```

The enum types may include the already canonical later states for decoding, but no
TASK-006 public mutation may transition to them. Retirement, missing/corrupt
observation, removal, GC and purge remain disabled.

### 5.3 Aggregate constructors

The application-facing domain constructors are semantic and consume only validated
typed values:

```rust
AssetGraph::register_managed(RegisterManagedAssetValues) -> Result<AssetGraph, AssetError>
Asset::create_revision(CreateAssetRevisionValues) -> Result<NewAssetRevision, AssetError>
BlobRecord::record_location(RecordManagedLocationValues) -> Result<LocationChange, AssetError>
```

`register_managed` requires exactly one Asset, initial sequence 1 revision, one
Representation, one Resource, exactly one Member at ordinal zero, the exact digest
of the single consumed `DurableBlob`, and that Blob's single durable managed
Location. No other Member digest is accepted. This closes `DATA-013` without
pretending that one custody authority proves multiple members. A future multi-member
registration MUST accept a bounded digest-keyed set of `DurableBlob` authorities and
receive a separate reviewed gate.

`create_revision` requires the current expected Asset revision, a non-empty unique
parent set from the same Asset, sequence equal to prior maximum plus one, and new
identities for revision/representation/resource/member rows. It never mutates prior
revision rows. `record_location` changes Blob/Location metadata only and never
creates an AssetRevision.

The domain model retains future-safe collection caps of at most 64 parents, 64
Representations per revision, 64 Resources per Representation and 4096 Members per
Resource, but TASK-006 production registration accepts only the exact one/one/one/
one graph above. Generalized graph construction is test/internal-only until a later
semantic operation supplies custody for every Member; cap+1 fails before store
submission.

### 5.4 Row separation and mapping

Domain objects, application requests, port DTOs and SQLite rows are separate types.
Every row mapper validates:

- UUIDv7 bytes through `Id::from_bytes`;
- digest bytes through `Sha256Digest`;
- revision through exact eight-byte big-endian `RevisionNo`;
- timestamps through `Timestamp::from_unix_seconds_nanos`;
- every enum/token and finite byte length;
- exactly one slash in a media type and `char::is_control() == false` for logical
  names, backend IDs and locators;
- nonnegative/checked sequence, byte length and event order;
- all conditional row invariants.

Conversion failure after schema validation is `STORAGE_CORRUPTION`; it is never a
lossy default or panic.

SQL provides storage class, byte length, NUL and the exact media-type slash/ASCII
floor. Unicode control-character rejection is intentionally a mapper/domain rule:
all production write DTO fields are private and can be constructed only through
those validators, the SQLite adapter exposes no raw write/SQL callback, and direct
corruption fixtures containing non-NUL controls MUST be rejected when materialized.

## 6. Application and port boundary

### 6.1 No transport or path authority

`mengxia-app` owns a crate-private, transport-neutral `AssetPersistenceService`. It
receives an already validated command binding and an opaque TASK-005 `DurableBlob`.
It never receives a source path, SQLite connection or CAS root/locator
interpretation. TASK-006 never calls
`DurableBlob::__from_verified_local_adapter`; the retained TASK-005 architecture
gate continues to permit that trusted construction seam only inside
`mengxia-storage-local`. The app consumes `DurableBlob` by value and converts its
read-only fields once into bounded persistence DTOs. TASK-006 exposes no public app
facade for TASK-007 to guess against: the only cross-crate mutation contract is the
exact `mengxia-ports` boundary below, while TASK-007 later adds its reviewed
transport-to-app orchestration inside `mengxia-app`.

`mengxia-ports` owns this exact object-safe asynchronous interface. Owned request
DTOs cross admission; the returned future borrows only the handle and dropping it
does not revoke an admitted job:

```rust
pub type AssetPortFuture<'a, T> = Pin<
    Box<dyn Future<Output = Result<T, AssetStoreError>> + Send + 'a>
>;

pub trait AssetUnitOfWork: Send + Sync {
    fn claim_external_ingest(
        &self,
        request: ExternalIngestClaim,
    ) -> AssetPortFuture<'_, ExternalClaimOutcome>;

    fn complete_external_ingest(
        &self,
        request: ExternalIngestCompletion,
    ) -> AssetPortFuture<'_, MutationOutcome>;

    fn finish_external_ingest(
        &self,
        request: ExternalIngestDisposition,
    ) -> AssetPortFuture<'_, ExternalDispositionOutcome>;

    fn fail_current_runtime_for_unresolved_external_ingest(&self);

    fn execute_create_revision(
        &self,
        request: CreateAssetRevisionCommand,
    ) -> AssetPortFuture<'_, MutationOutcome>;

    fn execute_record_location(
        &self,
        request: RecordManagedLocationCommand,
    ) -> AssetPortFuture<'_, MutationOutcome>;
}
```

The synchronous fail-current-runtime method is idempotent, performs no SQLite or
filesystem I/O and accepts no caller-controlled reason. Under the existing lifecycle
mutex it changes the shared gate to `Failed`, fails queued receipts with the static
internal result and rejects every later read/write admission; an already executing
writer is allowed to finish or roll back and remains joined by shutdown. Poisoned
lifecycle state already rejects admission and is equivalent to failure. The method
is an in-process application safety capability, never a transport operation.

Immediately before submitting an external claim, `AssetPersistenceService` arms a
private guard holding only an `Arc<dyn AssetUnitOfWork>` and a boolean. A claim
outcome proving that no new work is owned (`InProgress`, replay, terminal rejection,
recovery-required or binding error), a delivered claim error that proves no claim
committed or that the store gate is already failed, exact completion, or an exact
stored terminal/recovery disposition disarms it. After `Claimed`, every other
return, panic or future drop leaves it armed; `Drop` invokes
`fail_current_runtime_for_unresolved_external_ingest`. This makes post-claim
backpressure, completion/disposition receipt loss, post-CAS clock/ID failure and
caller cancellation fail closed without a detached cleanup task. Process death is
handled separately by the persisted runtime ID on restart.

`claim_external_ingest` may deliver `Err` only after its transaction is known rolled
back with no new CommandRecord, or after the store has already failed its own gate.
An uncertain commit/receipt path fails the store gate internally before returning;
dropping the claim future before a delivered outcome is independently covered by
the armed app guard. Completion/disposition errors never claim that the external
effect is absent and therefore do not disarm that guard.

`OperationId`, `CommandBinding`, every request/result DTO and `AssetUnitOfWork` are
owned by `mengxia-ports`. `OperationId` is a copyable private-field value with only
`as_str()` and the three crate-owned constant constructors below; it has no public
arbitrary-string constructor. `CommandBinding` has private fields, one checked
constructor accepting typed command ID, one of those `OperationId` values and the
canonical request digest, plus read-only accessors. The public operation constants
are exactly:

```rust
pub const ASSET_INGEST_COPY_V1: OperationId = OperationId::asset_ingest_v1();
pub const ASSET_REVISION_CREATE_V1: OperationId = OperationId::asset_revision_create_v1();
pub const BLOB_LOCATION_RECORD_V1: OperationId = OperationId::blob_location_record_v1();
```

All listed DTO fields are private and constructors validate operation-family and
shape before admission:

| DTO | Exact owned content |
|---|---|
| `ExternalIngestClaim` | binding fixed to `ASSET_INGEST_COPY_V1`; `claimed_at` |
| `ExternalIngestCompletion` | same binding; consumed `DurableBlob`; exact single-member `ManagedRegistrationPlan`; one domain-event ID; one provenance-event ID; `completed_at` |
| `ExternalIngestDisposition` | same binding; `TerminalRejected(ErrorCode)` or `RecoveryRequired(ErrorCode)` from the operation allowlist; `observed_at` |
| `CreateAssetRevisionCommand` | binding fixed to `ASSET_REVISION_CREATE_V1`; expected Asset revision; validated `NewAssetRevision` graph including parents/Representations/Resources/Members; one domain-event ID; one provenance-event ID; `operation_at` |
| `RecordManagedLocationCommand` | binding fixed to `BLOB_LOCATION_RECORD_V1`; consumed `DurableBlob`; candidate Location ID used only on insert; expected Blob revision; one domain-event ID; `operation_at` |

`ManagedRegistrationPlan` contains exactly: new Asset ID and `AssetKind`; new
AssetRevision ID and `ContentKind`; new Representation ID and
`RepresentationPurpose`; new Resource ID and `ResourceKind`; the sole Member logical
name; optional validated media type; and a candidate Location ID used only when the
descriptor does not already exist. ResourceMember has no
independent ID in the accepted model and is identified by Resource ID plus ordinal
zero. The plan contains no Blob digest, byte length, backend ID or locator; those are
derived exactly once from the consumed `DurableBlob`, and the sole Member is forced
to that digest. Lifecycle, custody, durability, verification, initial sequence,
ordinals and initial revisions are fixed by the operation and cannot be supplied.

The result enums are exact:

```rust
pub enum ExternalClaimOutcome {
    Claimed,
    InProgress,
    Replay(CommandResult),
    TerminalRejected { safe_error_code: ErrorCode },
    RecoveryRequired { safe_error_code: ErrorCode },
}

pub enum MutationOutcome {
    Applied(CommandResult),
    Replay(CommandResult),
    TerminalRejected { safe_error_code: ErrorCode },
    RecoveryRequired { safe_error_code: ErrorCode },
}

pub enum ExternalDispositionOutcome {
    Stored,
    Replay { safe_error_code: ErrorCode },
}

pub enum CommandResult {
    ManagedRegistration(ManagedRegistrationResult),
    AssetRevision(AssetRevisionResult),
    Location(LocationResult),
}
```

The method/result matrix is closed: external claim replay and external completion
accept only `ManagedRegistration`; creative revision accepts only `AssetRevision`;
Location recording accepts only `Location`. External completion may return
`Applied`, exact `Replay`, or a `RecoveryRequired` that it actually persisted from
the current claim without graph/event mutation. It may never return
`TerminalRejected`, because possession of a post-CAS `DurableBlob` disproves the
required no-physical-effect condition. Pure revision/Location methods may return
`Applied`, exact `Replay` or deterministic `TerminalRejected`, but never
`RecoveryRequired`. Any other operation/result/outcome combination is
`STORAGE_CORRUPTION`; it is not coerced into a nearby enum variant.

`ManagedRegistrationResult` contains exactly Asset, AssetRevision, Representation,
Resource and Location IDs plus Blob digest; `AssetRevisionResult` contains Asset ID,
new AssetRevision ID and resulting Asset `RevisionNo`; `LocationResult` contains Blob
digest, Location ID and resulting Blob `RevisionNo`. Fields are typed/private with
read-only accessors and contain no backend ID/locator/path. The command row stores
the root `result_kind + result_id` and, only for `ASSET`, the exact immutable
`result_location_id`. Replay reconstructs the other matching immutable graph fields
through fixed indexed queries in the same writer job, uses the stored Location ID
rather than selecting one of a Blob's current Locations, and treats any missing,
rebound or mismatched graph/result reference as `STORAGE_CORRUPTION`.

`AssetStoreError` is `#[non_exhaustive]`, stores no dynamic string, and has exactly
typed variants for Validation, NotFound, Conflict, InvalidTransition,
RevisionExhausted, IdGenerationUnavailable, StorageBusy, StorageIo,
StorageCorruption, StorageConfiguration, Backpressure, ShuttingDown and Internal.
Its `error_code()` mapping is fixed by §10. Recovery-required is an authorized command
outcome, not an error variant. The application maps `InProgress` to
`COMMAND_IN_PROGRESS` and maps both `TerminalRejected.safe_error_code` and
`RecoveryRequired.safe_error_code` exactly; it does not invent a second store error.

`ExternalIngestDisposition` accepts only these exact typed code sets:

```text
TerminalRejected:
  VALIDATION_ERROR, SOURCE_MODIFIED_DURING_INGEST, STORAGE_IO_ERROR,
  STORAGE_CORRUPTION, STORAGE_CONFIGURATION_ERROR, BACKPRESSURE,
  INTERNAL_ERROR, ID_GENERATION_UNAVAILABLE, DEADLINE_EXCEEDED,
  OPERATION_CANCELLED
RecoveryRequired:
  STORAGE_CONFIGURATION_ERROR, STORAGE_IO_ERROR, ID_GENERATION_UNAVAILABLE,
  INTERNAL_ERROR
```

An exact current-runtime claim may store one disposition. Repeating the identical
disposition returns `Replay`; a different disposition, a completed row, a prior-
runtime claim or any binding mismatch returns `CONFLICT` without overwriting. An
unexpected code is `VALIDATION_ERROR` before writer admission.
`TerminalRejected` additionally requires TASK-005's typed result to prove that no
durable effect remains and cleanup is not uncertain; any failed/uncertain cleanup,
lost completion ownership or post-CAS failure uses `RecoveryRequired`. The same
safe code may appear in both sets only because the durable state, not the string,
records whether reconciliation is required. A post-claim BACKPRESSURE disposition is
terminal for that command ID; a fresh attempt uses a new command ID.

TASK-005 outcomes map without guesswork: `Stopped(DeadlineReached)` is terminal
`DEADLINE_EXCEEDED`; `Stopped(Cancelled)` is terminal `OPERATION_CANCELLED`; and
pre-staging `EntropyUnavailable` is terminal `ID_GENERATION_UNAVAILABLE`. TASK-005
returns `Stopped` only after certain pre-promote cleanup, so neither stop result is
recovery-required. `CleanupFailed`, `RecoveryRequired`, exhausted/colliding staging
namespace and any otherwise uncertain physical effect use the recovery allowlist.
Unknown future non-exhaustive storage errors fail the current runtime rather than
being coerced into an existing code.

Callers never supply event type, aggregate kind/ID, aggregate revision, sequence,
principal or schema version. The store derives those fields from the operation
method and the state it actually commits. Caller-provided event IDs are only typed
identities. Registration emits exact `asset.registered.v1` and
`asset.ingested.copy.v1`; revision creation emits exact
`asset.revision.created.v1` and `asset.revision.derived.v1`; Location recording emits
exact `blob.location.recorded.v1` and no ProvenanceEvent. Any ID reuse or mismatch is
rejected before state mutation.

There is no recovery cursor/page or automatic recovered-completion method in
TASK-006. Stale claims are resolved only on exact-command lookup as §7 defines;
TASK-008 may later add bounded reporting APIs without changing this port silently.

There is no generic CRUD/repository method and no arbitrary SQL callback.

`OpenedLibrary::asset_store_handle()` returns a cloneable opaque
`SqliteAssetStoreHandle` that implements the port. The handle contains only the
existing bounded lifecycle handle and immutable Library metadata; it exposes no
Connection, SQL, path, lock or owner-UID constructor. Outstanding handles after
`OpenedLibrary::shutdown` can only receive the typed shutting-down result and cannot
extend filesystem/lock authority.

### 6.2 ID and clock sources

The app service owns crate-private injected `AssetIdentitySource` and `Clock` seams;
they are not cross-crate public API. Their exact methods are
`AssetIdentitySource::next_id<T>() -> Result<Id<T>, IdGenerationError>` and
`Clock::now() -> Result<Timestamp, IdGenerationError>`. The generic identity method
does not need to be object-safe because `AssetPersistenceService` is generic over
the two private seams; only `AssetUnitOfWork` is object-safe. Production defaults use
`Id::<T>::try_new()` and a checked `SystemTime` conversion; deterministic tests use
finite fakes. Time has three non-interchangeable boundaries:

1. external ingest samples `claimed_at` exactly once after source-selector digest
   construction and before its standalone claim transaction;
2. after CAS returns a verified `DurableBlob`, TASK-007 samples `completed_at`
   exactly once; it becomes Asset/Revision creation time, Blob/Location
   `verified_at`, event time and command update time;
3. each pure SQLite revision/location request samples one `operation_at` before
   writer submission and uses it for its mutation/events/outcome.

Pure-operation IDs and time are all sampled before admission, so failure leaves no
row. Registration Asset, AssetRevision, Representation, Resource, Location and event
IDs are sampled only after CAS succeeds; ResourceMember has no ID. If
post-CAS ID generation fails after `completed_at` exists, the app records
`RECOVERY_REQUIRED` using that time. If the completion clock itself fails, it cannot
fabricate a verification time: mutation admission fails closed for that store
runtime, restart creates a different runtime ID, and exact retry lazily yields
`RECOVERY_REQUIRED` while TASK-005 reports the orphan.

The store samples one internal UUIDv7 `store_runtime_id` before migration/readers/
workers. Failure returns `ID_GENERATION_UNAVAILABLE` before product mutation. This
ID is persisted only on CommandRecord and has no public constructor or authority.
Deterministic tests fail every individual ID/time sample and prove the exact
pre/post-effect disposition.

### 6.3 Cancellation and deadline

Store submission uses the already bounded TASK-004 writer queue. Dropping the
caller future after admission does not cancel an executing SQLite transaction. A
pre-admission expired/cancelled request is not submitted. Once admitted, the writer
commits or rolls back, stores one terminal result where applicable, and the caller
may recover by the same command ID.

No task or thread is detached. Store shutdown remains the sole queued-command
revocation point; every admitted/running job is joined before lock release.

## 7. Durable CommandRecord contract

### 7.1 Binding and precedence

`CommandBinding` contains exactly:

```text
command_id: Id<Command>
operation_id: OperationId
canonical_request_digest: Sha256Digest
```

The authenticated principal is not a field. The store injects
`LOCAL_OWNER_UID_V1 + OpenedLibraryMetadata.owner_uid`.

Binding lookup always precedes current-state validation. For external ingest claim:

1. no row: insert/commit `CLAIMED` with injected principal and current
   `store_runtime_id`, then return `Claimed`;
2. exact binding + `CLAIMED` from the current runtime: return `InProgress`;
3. exact binding + `CLAIMED` from another runtime: atomically change only that row
   to `RECOVERY_REQUIRED` with `STORAGE_CONFIGURATION_ERROR`, then return the
   recovery-required outcome; do not execute or infer a physical effect;
4. exact binding + `COMPLETED`: return `Replay(stored typed result)`;
5. exact binding + `TERMINAL_REJECTED`: return
   `TerminalRejected { safe_error_code }` with the stored typed safe error;
6. exact binding + `RECOVERY_REQUIRED`: return the stored recovery-required result;
7. any operation/principal/digest mismatch: `CONFLICT`, with no prior result or
   object-existence disclosure and no row mutation.

For pure SQLite operations, the same precedence is evaluated inside their sole
mutation transaction. No-row inserts the uncommitted claim and continues; success
commits `COMPLETED`, while a deterministic domain/expected-revision rejection commits
only `TERMINAL_REJECTED`. Exact completed/rejected outcomes replay and a binding
mismatch conflicts. Because a pure operation can never legitimately commit
`CLAIMED` or `RECOVERY_REQUIRED`, observing either for its operation ID is
`STORAGE_CORRUPTION`, not `COMMAND_IN_PROGRESS`.

No command row expires in TASK-006. No retry loop, startup command scan, recovered
completion or reset-to-fresh-claim exists. A current-runtime external operation that
cannot either complete or store a terminal disposition MUST close mutation
admission; it cannot leave an apparently live claim while accepting new work.

### 7.2 Atomic completion

For each pure SQLite operation, one `BEGIN IMMEDIATE` transaction performs:

```text
lookup/replay/conflict or insert the uncommitted exact binding
validate expected revision/current rows
insert/update canonical state
allocate event sequence(s)
append provenance/domain event(s)
update command to COMPLETED with result_ref
re-read observable result/invariants
COMMIT
```

Infrastructure, mapping, statement or unexpected constraint failure rolls back all
listed effects and the fresh command row. A deterministic business rejection may
commit only its terminal CommandRecord error, with no state/event. A successful
mutation commit followed by caller disconnect is recovered by exact command replay.
No event may exist without its state mutation and no state mutation may commit
without its DomainEvent.

External ingest is the sole split transaction: transaction A commits `CLAIMED`, CAS
runs outside SQLite under TASK-007, and transaction B re-reads the exact current-
runtime claim then atomically registers state/events/`COMPLETED`. Transaction B may
never accept a different-runtime or `RECOVERY_REQUIRED` claim. Crash/failure after
CAS but before B leaves no Asset and is reported as recovery-required/orphan, never
auto-completed from caller-reconstructed graph data.

Each TASK-006 mutation requests exactly one DomainEvent sequence. The shared helper
also supports a bounded future batch of 1..64 sequences with one statement equivalent
to `UPDATE event_commit_sequence SET last_sequence = last_sequence + :count WHERE
singleton = 1 AND last_sequence <= 9223372036854775807 - :count RETURNING
last_sequence`. Zero returned rows is `REVISION_EXHAUSTED`; the first allocated value
is `returned_last - count + 1`. Allocation and event inserts stay inside the same
transaction, so rollback cannot consume a sequence.

## 8. Migration 0001 contract

### 8.1 Identity and immutable bytes

```text
filename: migrations/sqlite/0001_library_assets.sql
stored migration_name: 0001_library_assets
migration_sequence: 1
checksum algorithm: SHA-256 over the exact UTF-8 file bytes, including final LF
candidate byte length: 12733
candidate SHA-256: 91c76e615fe248abd852860dcd42b32a01f6f024e91ac8387f34069be2435db1
application timestamp: one checked pre-transaction UTC sample
```

The candidate digest was independently reproduced with `/usr/bin/shasum -a 256` and
`/usr/bin/openssl dgst -sha256`. The pinned SQLite 3.53.4 CLI parsed the complete
candidate DDL with `foreign_keys=ON`; `quick_check` returned `ok` and
`foreign_key_check` returned no row. A representative claimed-command → complete
managed graph/event transaction and a pure claim+revision+events+outcome transaction
also passed under `trusted_schema=OFF`. Multi-slash media type, self-correction and
raw ProvenanceEvent update were rejected by their exact constraints/triggers. The
implementation MUST prove the checked-in
migration has these exact bytes before compilation and at runtime.

### 8.2 Upgrade state machine

While holding the TASK-004 durable Library lock and before worker startup:

| Observed state | Result |
|---|---|
| exact 0000 schema/row only | apply exact 0001 in one IMMEDIATE transaction, insert migration row, validate, commit |
| exact 0000+0001 schema/rows | validate and open; do not rewrite/reapply |
| transaction rolled back after kill/fault | exact 0000 remains; retry once on next startup |
| commit durable but reply/checkpoint absent | SQLite WAL recovery yields exact 0001; validate and open |
| row/schema/checksum gap, partial object, extra known-version object | `STORAGE_CORRUPTION`, no mutation |
| contiguous syntactically valid migration sequence above compiled maximum | `STORAGE_CONFIGURATION_ERROR` (newer software required), no mutation |
| unknown/invalid name, checksum, duplicate sequence/name | `STORAGE_CORRUPTION`, no mutation |

Migration application has no automatic busy retry beyond the accepted connection
busy timeout. `BUSY` maps to `STORAGE_BUSY`; I/O/full/readonly maps to
`STORAGE_IO_ERROR`; constraint/schema misuse inside the embedded known migration is
`INTERNAL_ERROR` and prevents runtime admission.

### 8.3 Schema object set

Migration 0001 creates exactly these new tables:

```text
event_commit_sequence
commands
assets
asset_revisions
asset_revision_parents
representations
resources
resource_members
blobs
locations
provenance_events
domain_events
```

It also creates only the explicit indexes and append-only triggers listed by the
accepted SQL. Every foreign-key child lookup has a covering index. No view, virtual
table, generated column, FTS object or provider-specific field is allowed.

### 8.4 Normative column rules

All new tables are `STRICT`. IDs are BLOB(16); Blob identity is BLOB(32);
`RevisionNo` is BLOB(8) big-endian; timestamps are signed seconds plus nanos in
0..999,999,999. UUID semantic validity is enforced by row mappers. All delete/update
foreign-key actions default to `NO ACTION`; TASK-006 has no cascade deletion.

The exact SQL is frozen by Appendix A for this review candidate. The following
column/index contract is normative for the draft and may not be weakened:

| Table | Primary/unique identity | Required integrity |
|---|---|---|
| event_commit_sequence | singleton=1 | checked last sequence 0..i64::MAX |
| commands | command_id | operation + store-derived principal + digest + creating-runtime binding; conditional state/result/error columns; successful Asset result pins its exact Location ID |
| assets | asset_id | typed kind; ACTIVE; revision; creator/time |
| asset_revisions | revision_id; unique(asset_id,sequence); unique(asset_id,revision_id) | immutable sequence/content/custody/creator/time |
| asset_revision_parents | (child_revision_id,ordinal); unique(child,parent) | composite FKs prove same Asset; max 64 enforced in app |
| representations | representation_id | FK revision; bounded purpose |
| resources | resource_id | FK representation; bounded kind |
| resource_members | (resource_id,ordinal) | FK Blob; bounded logical name; max 4096 in app |
| blobs | digest | exact byte length; AVAILABLE; revision; optional bounded media type; verified time |
| locations | location_id; unique(backend_id,locator) | FK Blob; opaque bounded descriptor; MANAGED/DURABLE/AVAILABLE; revision/time |
| provenance_events | event_id | command/revision/blob links; schema/type/verification/time/correction; append-only |
| domain_events | event_id; unique(commit_sequence) | command, aggregate reference, revision and time; append-only |

### 8.5 Migration, reopen and connection validation cost

The migration-capable writer performs exactly once per Library open:

- exact SQLite runtime hardening assertions already owned by TASK-004;
- TASK-004 `quick_check`; it may inspect database pages proportional to metadata DB
  size and is not described as bounded/constant-time;
- exact ordered migration rows, names and checksums for 0000 and 0001;
- exact `sqlite_schema` allowlist including autoindexes, explicit indexes and
  append-only triggers;
- exact table/index/foreign-key contracts through bounded PRAGMA result shapes;
- typed singleton allocator row and forced-index probes for migration names,
  command binding, asset sequence, Location identity and event sequence.

Immediately after applying 0001, while all newly created product tables are empty,
the same transaction runs full `foreign_key_check` and rejects any row. On an
already-current normal reopen it does not run full `foreign_key_check` and does not
materialize every domain row. Each reader connection validates only its own runtime/
hardening settings plus the exact Library identity and migration-prefix metadata; it
does not repeat quick/FK/schema traversal. Every production mutation keeps
`foreign_keys=ON`, and every materialized row passes typed mapping.

TASK-008 owns explicit deep FK/domain traversal and progress/reporting policy. A
malformed row reached by TASK-006 operations still maps immediately to
`STORAGE_CORRUPTION`; deferring the whole-row scan does not authorize repair or
lossy decoding.

### 8.6 Appendix A — exact candidate migration bytes

The UTF-8 bytes inside the following single SQL fence, with LF line endings and one
final LF, are the exact candidate `0001_library_assets.sql`. The digest recorded in
§8.1 is calculated over those bytes only. Review corrections that change one byte
MUST update the digest in the same change.

```sql
CREATE TABLE event_commit_sequence (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    last_sequence INTEGER NOT NULL CHECK (last_sequence BETWEEN 0 AND 9223372036854775807)
) STRICT;

INSERT INTO event_commit_sequence (singleton, last_sequence) VALUES (1, 0);

CREATE TABLE commands (
    command_id BLOB PRIMARY KEY NOT NULL CHECK (length(command_id) = 16),
    operation_id TEXT NOT NULL
        CHECK (length(CAST(operation_id AS BLOB)) BETWEEN 1 AND 128)
        CHECK (substr(operation_id, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (operation_id NOT GLOB '*[^a-z0-9._-]*')
        CHECK (operation_id GLOB '*.v1'),
    principal_kind TEXT NOT NULL CHECK (principal_kind = 'LOCAL_OWNER_UID_V1'),
    principal_uid INTEGER NOT NULL CHECK (principal_uid BETWEEN 0 AND 4294967295),
    canonical_request_digest BLOB NOT NULL CHECK (length(canonical_request_digest) = 32),
    store_runtime_id BLOB NOT NULL CHECK (length(store_runtime_id) = 16),
    state TEXT NOT NULL
        CHECK (state IN ('CLAIMED', 'COMPLETED', 'TERMINAL_REJECTED', 'RECOVERY_REQUIRED')),
    result_kind TEXT
        CHECK (result_kind IS NULL OR result_kind IN ('ASSET', 'ASSET_REVISION', 'LOCATION')),
    result_id BLOB CHECK (result_id IS NULL OR length(result_id) = 16),
    result_location_id BLOB
        CHECK (result_location_id IS NULL OR length(result_location_id) = 16),
    safe_error_code TEXT
        CHECK (safe_error_code IS NULL OR length(CAST(safe_error_code AS BLOB)) BETWEEN 1 AND 64),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    updated_at_seconds INTEGER NOT NULL,
    updated_at_nanos INTEGER NOT NULL CHECK (updated_at_nanos BETWEEN 0 AND 999999999),
    CHECK (
        (state = 'CLAIMED' AND result_kind IS NULL AND result_id IS NULL
            AND result_location_id IS NULL AND safe_error_code IS NULL)
        OR (state = 'COMPLETED' AND result_kind IS NOT NULL AND result_id IS NOT NULL
            AND safe_error_code IS NULL
            AND ((result_kind = 'ASSET' AND result_location_id IS NOT NULL)
                 OR (result_kind IN ('ASSET_REVISION', 'LOCATION')
                     AND result_location_id IS NULL)))
        OR (state IN ('TERMINAL_REJECTED', 'RECOVERY_REQUIRED')
            AND result_kind IS NULL AND result_id IS NULL
            AND result_location_id IS NULL AND safe_error_code IS NOT NULL)
    ),
    FOREIGN KEY (result_location_id) REFERENCES locations(location_id)
) STRICT;

CREATE TABLE assets (
    asset_id BLOB PRIMARY KEY NOT NULL CHECK (length(asset_id) = 16),
    kind TEXT NOT NULL
        CHECK (length(CAST(kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(kind, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (kind NOT GLOB '*[^a-z0-9._-]*'),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('ACTIVE', 'RETIRED')),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    created_by_uid INTEGER NOT NULL CHECK (created_by_uid BETWEEN 0 AND 4294967295)
) STRICT;

CREATE TABLE asset_revisions (
    asset_revision_id BLOB PRIMARY KEY NOT NULL CHECK (length(asset_revision_id) = 16),
    asset_id BLOB NOT NULL CHECK (length(asset_id) = 16),
    sequence INTEGER NOT NULL CHECK (sequence BETWEEN 1 AND 4294967295),
    content_kind TEXT NOT NULL
        CHECK (length(CAST(content_kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(content_kind, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (content_kind NOT GLOB '*[^a-z0-9._-]*'),
    custody TEXT NOT NULL CHECK (custody IN ('MANAGED', 'UNMANAGED')),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    created_by_uid INTEGER NOT NULL CHECK (created_by_uid BETWEEN 0 AND 4294967295),
    FOREIGN KEY (asset_id) REFERENCES assets(asset_id),
    UNIQUE (asset_id, sequence),
    UNIQUE (asset_id, asset_revision_id)
) STRICT;

CREATE TABLE asset_revision_parents (
    asset_id BLOB NOT NULL CHECK (length(asset_id) = 16),
    child_revision_id BLOB NOT NULL CHECK (length(child_revision_id) = 16),
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 63),
    parent_revision_id BLOB NOT NULL CHECK (length(parent_revision_id) = 16),
    PRIMARY KEY (child_revision_id, ordinal),
    UNIQUE (child_revision_id, parent_revision_id),
    CHECK (child_revision_id <> parent_revision_id),
    FOREIGN KEY (asset_id, child_revision_id)
        REFERENCES asset_revisions(asset_id, asset_revision_id),
    FOREIGN KEY (asset_id, parent_revision_id)
        REFERENCES asset_revisions(asset_id, asset_revision_id)
) STRICT;

CREATE TABLE representations (
    representation_id BLOB PRIMARY KEY NOT NULL CHECK (length(representation_id) = 16),
    asset_revision_id BLOB NOT NULL CHECK (length(asset_revision_id) = 16),
    purpose TEXT NOT NULL
        CHECK (length(CAST(purpose AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(purpose, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (purpose NOT GLOB '*[^a-z0-9._-]*'),
    FOREIGN KEY (asset_revision_id) REFERENCES asset_revisions(asset_revision_id)
) STRICT;

CREATE TABLE resources (
    resource_id BLOB PRIMARY KEY NOT NULL CHECK (length(resource_id) = 16),
    representation_id BLOB NOT NULL CHECK (length(representation_id) = 16),
    kind TEXT NOT NULL
        CHECK (length(CAST(kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(kind, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (kind NOT GLOB '*[^a-z0-9._-]*'),
    FOREIGN KEY (representation_id) REFERENCES representations(representation_id)
) STRICT;

CREATE TABLE blobs (
    digest BLOB PRIMARY KEY NOT NULL CHECK (length(digest) = 32),
    byte_length INTEGER NOT NULL CHECK (byte_length BETWEEN 0 AND 1099511627776),
    media_type TEXT
        CHECK (media_type IS NULL OR (
            length(CAST(media_type AS BLOB)) BETWEEN 3 AND 255
            AND instr(media_type, '/') BETWEEN 2 AND length(media_type) - 1
            AND instr(substr(media_type, instr(media_type, '/') + 1), '/') = 0
            AND media_type NOT GLOB '*[^a-z0-9!#$&^_.+/-]*'
        )),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('AVAILABLE', 'GC_PENDING', 'PURGED')),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    verified_at_seconds INTEGER NOT NULL,
    verified_at_nanos INTEGER NOT NULL CHECK (verified_at_nanos BETWEEN 0 AND 999999999)
) STRICT;

CREATE TABLE resource_members (
    resource_id BLOB NOT NULL CHECK (length(resource_id) = 16),
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4095),
    logical_name TEXT NOT NULL
        CHECK (length(CAST(logical_name AS BLOB)) BETWEEN 0 AND 255)
        CHECK (instr(logical_name, char(0)) = 0),
    blob_digest BLOB NOT NULL CHECK (length(blob_digest) = 32),
    PRIMARY KEY (resource_id, ordinal),
    FOREIGN KEY (resource_id) REFERENCES resources(resource_id),
    FOREIGN KEY (blob_digest) REFERENCES blobs(digest)
) STRICT;

CREATE TABLE locations (
    location_id BLOB PRIMARY KEY NOT NULL CHECK (length(location_id) = 16),
    blob_digest BLOB NOT NULL CHECK (length(blob_digest) = 32),
    backend_id TEXT NOT NULL
        CHECK (length(CAST(backend_id AS BLOB)) BETWEEN 1 AND 255)
        CHECK (instr(backend_id, char(0)) = 0),
    locator TEXT NOT NULL
        CHECK (length(CAST(locator AS BLOB)) BETWEEN 1 AND 1024)
        CHECK (instr(locator, char(0)) = 0),
    custody TEXT NOT NULL CHECK (custody IN ('MANAGED', 'UNMANAGED')),
    durability TEXT NOT NULL CHECK (durability IN ('DURABLE', 'UNKNOWN')),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('AVAILABLE', 'CORRUPT', 'MISSING', 'REMOVED')),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    verified_at_seconds INTEGER NOT NULL,
    verified_at_nanos INTEGER NOT NULL CHECK (verified_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (blob_digest) REFERENCES blobs(digest),
    UNIQUE (backend_id, locator)
) STRICT;

CREATE TABLE provenance_events (
    provenance_event_id BLOB PRIMARY KEY NOT NULL CHECK (length(provenance_event_id) = 16),
    command_id BLOB NOT NULL CHECK (length(command_id) = 16),
    event_type TEXT NOT NULL
        CHECK (length(CAST(event_type AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(event_type, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (event_type NOT GLOB '*[^a-z0-9._-]*'),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    asset_revision_id BLOB NOT NULL CHECK (length(asset_revision_id) = 16),
    blob_digest BLOB CHECK (blob_digest IS NULL OR length(blob_digest) = 32),
    verification TEXT NOT NULL
        CHECK (verification IN ('UNKNOWN', 'VERIFIED', 'CONFLICTED', 'REJECTED')),
    occurred_at_seconds INTEGER NOT NULL,
    occurred_at_nanos INTEGER NOT NULL CHECK (occurred_at_nanos BETWEEN 0 AND 999999999),
    recorded_at_seconds INTEGER NOT NULL,
    recorded_at_nanos INTEGER NOT NULL CHECK (recorded_at_nanos BETWEEN 0 AND 999999999),
    correction_of BLOB CHECK (correction_of IS NULL OR length(correction_of) = 16),
    CHECK (correction_of IS NULL OR correction_of <> provenance_event_id),
    FOREIGN KEY (command_id) REFERENCES commands(command_id),
    FOREIGN KEY (asset_revision_id) REFERENCES asset_revisions(asset_revision_id),
    FOREIGN KEY (blob_digest) REFERENCES blobs(digest),
    FOREIGN KEY (correction_of) REFERENCES provenance_events(provenance_event_id)
) STRICT;

CREATE TABLE domain_events (
    domain_event_id BLOB PRIMARY KEY NOT NULL CHECK (length(domain_event_id) = 16),
    commit_sequence INTEGER NOT NULL UNIQUE
        CHECK (commit_sequence BETWEEN 1 AND 9223372036854775807),
    command_id BLOB NOT NULL CHECK (length(command_id) = 16),
    event_type TEXT NOT NULL
        CHECK (length(CAST(event_type AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(event_type, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (event_type NOT GLOB '*[^a-z0-9._-]*'),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    aggregate_kind TEXT NOT NULL
        CHECK (aggregate_kind IN ('ASSET', 'ASSET_REVISION', 'BLOB', 'LOCATION')),
    aggregate_id BLOB NOT NULL CHECK (length(aggregate_id) IN (16, 32)),
    aggregate_revision BLOB CHECK (aggregate_revision IS NULL OR length(aggregate_revision) = 8),
    occurred_at_seconds INTEGER NOT NULL,
    occurred_at_nanos INTEGER NOT NULL CHECK (occurred_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (command_id) REFERENCES commands(command_id),
    CHECK ((aggregate_kind = 'BLOB' AND length(aggregate_id) = 32)
           OR (aggregate_kind <> 'BLOB' AND length(aggregate_id) = 16))
) STRICT;

CREATE INDEX commands_state_idx
    ON commands(state, command_id);
CREATE INDEX commands_result_location_idx
    ON commands(result_location_id);
CREATE INDEX asset_revision_parents_child_asset_idx
    ON asset_revision_parents(asset_id, child_revision_id, ordinal);
CREATE INDEX asset_revision_parents_parent_asset_idx
    ON asset_revision_parents(asset_id, parent_revision_id, child_revision_id);
CREATE INDEX representations_revision_idx
    ON representations(asset_revision_id, representation_id);
CREATE INDEX resources_representation_idx
    ON resources(representation_id, resource_id);
CREATE INDEX resource_members_blob_idx
    ON resource_members(blob_digest, resource_id, ordinal);
CREATE INDEX locations_blob_backend_idx
    ON locations(blob_digest, backend_id, location_id);
CREATE INDEX locations_lifecycle_idx
    ON locations(lifecycle, location_id);
CREATE INDEX provenance_events_command_idx
    ON provenance_events(command_id, provenance_event_id);
CREATE INDEX provenance_events_revision_idx
    ON provenance_events(asset_revision_id, provenance_event_id);
CREATE INDEX provenance_events_blob_idx
    ON provenance_events(blob_digest, provenance_event_id);
CREATE INDEX provenance_events_correction_idx
    ON provenance_events(correction_of, provenance_event_id);
CREATE INDEX domain_events_command_idx
    ON domain_events(command_id, commit_sequence);
CREATE INDEX domain_events_aggregate_idx
    ON domain_events(aggregate_kind, aggregate_id, commit_sequence);

CREATE TRIGGER provenance_events_no_update
BEFORE UPDATE ON provenance_events
BEGIN
    SELECT RAISE(ABORT, 'provenance events are append-only');
END;

CREATE TRIGGER provenance_events_no_delete
BEFORE DELETE ON provenance_events
BEGIN
    SELECT RAISE(ABORT, 'provenance events are append-only');
END;

CREATE TRIGGER domain_events_no_update
BEFORE UPDATE ON domain_events
BEGIN
    SELECT RAISE(ABORT, 'domain events are append-only');
END;

CREATE TRIGGER domain_events_no_delete
BEFORE DELETE ON domain_events
BEGIN
    SELECT RAISE(ABORT, 'domain events are append-only');
END;
```

## 9. Persistence operations and invariants

### 9.1 Managed registration

`complete_external_ingest` accepts only a consumed TASK-005 `DurableBlob`, the exact
single-member plan and an exact current-runtime external claim. In one transaction it:

- reuses or inserts `blobs` by digest;
- if the Blob exists, requires byte length equality and compatible immutable facts;
- reuses an exact `(backend_id, locator)` only when it already names the same digest
  with identical durable managed state; any local-CAS locator naming another digest
  is uniquely `STORAGE_CORRUPTION`;
- always inserts new Asset, AssetRevision, Representation and Resource identities;
- inserts exactly one ordinal-zero Member→the consumed Blob and its Location→Blob FK;
- appends one `asset.registered.v1` DomainEvent and one
  `asset.ingested.copy.v1` ProvenanceEvent;
- completes the CommandRecord with result kind `ASSET`, the new Asset ID and the
  exact selected new-or-reused Location ID; the plan's candidate ID is used only on
  insert. The FK-backed
  result reference remains unambiguous after more Locations are recorded.

Two different commands with the same Blob bytes therefore share one Blob row but
have distinct Assets and creative graph identities.

### 9.2 New creative revision

The operation requires exact `expected_revision`. It uses a conditional Asset update
from old eight-byte revision to `checked_next()`, inserts a new immutable sequence
and its same-Asset parent links, appends the fixed Domain/Provenance events and
performs fresh command claim through completion in one transaction. Zero updated
rows is `CONFLICT`; exhausted revision/sequence is
`REVISION_EXHAUSTED`. No old revision/member/provenance/event row is updated.
Every new Member digest must already resolve in the same transaction to an AVAILABLE
Blob with at least one MANAGED/DURABLE/AVAILABLE Location; a missing authorized Blob
is `NOT_FOUND`, while an existing Blob lacking that custody invariant is
`STORAGE_CORRUPTION`. The bounded Member cap bounds these indexed proofs.

### 9.3 Location/metadata change

Recording another verified durable Location requires an exact expected Blob
revision and the consumed `DurableBlob`; it performs fresh command claim, Blob/
Location mutation, fixed DomainEvent and outcome in one transaction. It increments
only the Blob revision and creates no AssetRevision. Exact replay is idempotent. A
local-CAS locator already bound to another digest is `STORAGE_CORRUPTION`; expected-
revision and legitimate business uniqueness races are `CONFLICT`. Both commit no
change.

## 10. Error and safety contract

TASK-006 adds exactly one stable ErrorCode string, `OPERATION_CANCELLED`, because the
accepted TASK-005 `IngestStop::Cancelled` must have an honest durable replay result.
It is an additive `ErrorCode::OperationCancelled` parser/display mapping and changes
no existing code or retry meaning. All other typed domain/port/store variants map to
existing taxonomy rows:

```text
stable code: OPERATION_CANCELLED
source: cooperative caller cancellation completed before the physical effect
retry: no retry of the terminal command; a fresh attempt requires a new command ID
safe client detail: operation was cancelled
severity: INFO
metric: operation_cancelled_total
```

| Condition | ErrorCode | Retry rule |
|---|---|---|
| invalid token/shape/cap | VALIDATION_ERROR | after input change |
| missing authorized object | NOT_FOUND | no automatic retry |
| command binding/expected revision/legitimate business uniqueness mismatch | CONFLICT | reread/new semantic command only |
| durable claim still active | COMMAND_IN_PROGRESS | bounded caller delay; same command only |
| cooperative cancellation completed before promote with certain cleanup | OPERATION_CANCELLED | new command ID for a fresh attempt |
| propagated deadline stopped before promote with certain cleanup | DEADLINE_EXCEEDED | new command ID after deadline change |
| prior-runtime external claim / explicit uncertain effect | STORAGE_CONFIGURATION_ERROR outcome | operator orphan reconciliation; never auto-complete |
| illegal lifecycle request | INVALID_TRANSITION | no |
| revision/event sequence exhausted | REVISION_EXHAUSTED | no |
| ID/clock source failure | ID_GENERATION_UNAVAILABLE | after platform condition changes |
| SQLite BUSY | STORAGE_BUSY | fresh bounded admission only |
| SQLite/file I/O/full/readonly | STORAGE_IO_ERROR | typed/contextual; no store loop |
| schema/FK/row/checksum/type corruption, Blob fact mismatch or local-CAS locator cross-digest alias | STORAGE_CORRUPTION | no automatic retry |
| writer/read queue full | BACKPRESSURE | fresh bounded admission |
| shutdown before admission | STORAGE_IO_ERROR | new runtime only |
| unexpected UNIQUE/CHECK/FK after validated inputs, panic, LOCKED/MISUSE or invariant bug | INTERNAL_ERROR | close mutation admission; operator diagnosis |

Errors and Debug/Display MUST contain no SQL, path, UID, backend locator, digest,
member name, raw input, SQLite message or arbitrary payload. Authorized result DTOs
may return typed IDs/digest, but error text may not disclose existence after a
binding mismatch.

Security applicability:

- authentication is not reimplemented; owner principal is derived from the opened
  Library and TASK-003 remains the future transport source;
- V1 remains single-Library/single-owner, not tenant-isolated;
- no secret is accepted or persisted; every externally influenced value is
  authenticated where applicable and typed/bounded before queue admission;
- no timeout/retry loop or detached work is added;
- SQL parameters are bound; identifiers/SQL are compile-time constants;
- no destructive behavior exists.

## 11. Test and fault matrix

The accepted implementation MUST add deterministic seams for the following without
exposing them in production APIs:

### 11.1 Migration/reopen

- fresh 0000→0001 and already-current reopen;
- failure before/after each DDL batch, migration-row insert, validation and commit;
- SIGKILL before commit, after commit return and before checkpoint/worker start;
- missing/duplicate/gapped/wrong-name/wrong-checksum migration rows;
- extra/missing/modified table/index/trigger/view and column/FK/index shape;
- unknown newer contiguous sequence maps configuration, malformed unknown maps
  corruption;
- quick-check once per Library open; post-0001 full FK check; no normal-reopen or
  per-reader FK traversal; reader hardening/identity checks are still exact;
- malformed row and forced-index corruption;
- exact 0000 bytes/checksum remain unchanged.

### 11.2 Domain/mapping

- every ID marker mismatch is covered by a `compile_fail` rustdoc test, requiring no
  fixture manifest or test dependency;
- token and collection cap−1/cap/cap+1;
- UUID/digest/revision/timestamp/enum/conditional-row corruption;
- invalid store-runtime UUID and impossible operation/state/result combinations;
- same-Asset parent enforcement, duplicate parent/ordinal and cycle/self-parent
  rejection;
- ResourceMember ordinal uniqueness/contiguity;
- SQL rejects multi-slash media types; mapper rejects every remaining invalid media
  type and Unicode control character before persistence, while direct corrupt-row
  fixtures fail typed reads;
- arbitrary metadata/path rejected before persistence.

### 11.3 Commands/concurrency/events

- `asset.ingest.v1` digest golden vectors cover every semantic field, normalized raw
  path byte length/content, optional-digest presence, ambiguity and two-source
  non-aliasing without storing/logging the selector;
- external new claim, current-runtime in-progress, completed/rejected replay,
  prior-runtime lazy recovery-required and every binding mismatch;
- registration replay returns its pinned Location after a second Location is added
  to the same Blob, and missing/rebound result Location is corruption;
- no automatic recovered completion and no recovery graph/cursor public API;
- pure revision/location fresh claim+mutation+events+outcome are one transaction and
  never expose committed CLAIMED; statement faults leave no command/state/event;
- pure deterministic business rejection commits only the exact terminal error and
  replays it, with no state/event;
- same external command concurrent submit: one claim/effect, one in-progress or replay;
- same pure command concurrent submit: one complete mutation and one replay;
- different commands/same Blob: two Assets, one Blob;
- registration rejects zero/two Members and any Member digest not equal to the sole
  consumed DurableBlob;
- expected revision winner/loser and revision exhaustion;
- caller receipt dropped before/after admission and concurrent shutdown;
- failure after each state/event/command statement rolls back all;
- claim/completion/pure timestamp and every ID failure boundary, including
  post-CAS clock failure closing the current runtime;
- claim receipt drop and every unresolved-guard drop fail the current runtime,
  drain queued receipts and admit no later job;
- TASK-005 stopped deadline/cancellation and pre-staging entropy map exactly to
  terminal `DEADLINE_EXCEEDED`, `OPERATION_CANCELLED` and
  `ID_GENERATION_UNAVAILABLE`; uncertain cleanup stays recovery-required;
- event allocator rollback, contiguous multi-event allocation, overflow;
- raw UPDATE/DELETE of provenance/domain events rejected by exact triggers;
- no AssetRevision from Location/metadata mutation.

### 11.4 Security/architecture/supply

- no raw SQL/Connection/row/path/UID constructor exposed;
- exact object-safe port signatures, owned DTOs, result/error enums and operation
  constants compile; no caller event-type/aggregate/revision field exists;
- `OperationId` has no arbitrary constructor, app generation seams remain
  crate-private, ResourceMember has no invented ID and every candidate Location ID
  is sampled by the app rather than the store;
- no TASK-006 call to the hidden DurableBlob construction seam, with the retained
  TASK-005 constructor-ownership lint still passing;
- app/domain/events/ports remain free of rusqlite/Tokio concrete channels;
- store is the only SQLite adapter and keeps the single writer;
- no proto/bin/storage-local production change or later symbol;
- no dependency/feature/lockfile delta; any future need is a new blocker and requires
  proposal review before the authorized file scope can change;
- static-error canaries include SQL, path, locator, digest, UID and metadata text;
- retained TASK-001 through TASK-005 gates pass without weakening or silent skip.

## 12. Proposed stable acceptance registry

These IDs are candidates until canonical acceptance. On acceptance they become
stable and MUST be defined once in Specification §19.

```gherkin
AC-082
Given typed Asset graph values at every accepted boundary
When domain construction and row mapping run
Then Asset, AssetRevision and Blob identities remain distinct
And all IDs, tokens, revisions, timestamps, collections and states are validated
without infrastructure types or lossy/default conversion.

AC-083
Given an exact 0000 Library, exact current 0001 Library or any migration/schema
fault state
When startup migration and current-schema validation run
Then 0001 applies once transactionally or the exact current schema opens
And partial, tampered, gapped, duplicate or unsupported states fail closed before
product worker admission without weakening bootstrap recovery.

AC-084
Given two intended Assets backed by the same verified DurableBlob
When both managed registrations commit
Then Asset/Revision/Representation/Resource identities differ
And each registration has exactly one ordinal-zero Member bound to its consumed
DurableBlob while exactly one compatible Blob row is shared.

AC-085
Given a command ID and its operation, store-derived principal and request digest
When claim, duplicate, completion, rejection or recovery is evaluated
Then exactly one durable binding exists
And only external ingest may expose a current-runtime in-progress claim
And pure SQLite operations claim, mutate, emit and complete in one transaction
And terminal rejection and recovery-required outcomes are exactly representable
And a prior-runtime external claim becomes RECOVERY_REQUIRED without inferred
completion while every binding mismatch returns CONFLICT without disclosure.

AC-086
Given concurrent creative or Blob metadata mutations
When expected revision is checked
Then exactly one matching mutation may advance the big-endian RevisionNo
And losing, overflow, wrong-parent, duplicate-sequence and invalid-transition cases
commit no state or event.

AC-087
Given any accepted canonical state mutation
When its writer transaction commits or rolls back
Then state, CommandRecord outcome, required operation-specific provenance and
DomainEvents share one atomic boundary except the explicitly separate pre-CAS
external claim intent
And event IDs/order are immutable, per-Library monotonic and append-only.

AC-088
Given an opaque TASK-005 DurableBlob and Location descriptor
When TASK-006 persists or replays it
Then bytes/path authority never enter SQLite/domain/protocol/logs
And the source selector contributes only to the request digest
And backend/locator remain bounded opaque values bound to the exact digest while
Location-only change creates no creative revision
And replay returns the exact pinned original Location even after another Location
is recorded for that Blob.

AC-089
Given queue saturation, caller cancellation, SQLite failure, panic or shutdown
When a TASK-006 job is submitted/admitted/executed
Then the existing finite queue and joined lifecycle produce one recoverable terminal
disposition with no detached transaction, worker or Library authority
And any unresolved current-runtime external claim fails later admission closed.

AC-090
Given the complete TASK-006 candidate diff and retained repository
When architecture, supply-chain and document gates run
Then only the accepted domain/event/port/app/store/migration scope exists
And no TASK-007 transport/ingest orchestration, destructive or later capability,
unsafe expansion, unpinned dependency or completed-task regression is present.
```

Existing cross-task ownership after acceptance:

```text
AC-002 contributors: TASK-005, TASK-006; terminal owner TASK-007
AC-005 contributors: TASK-006; terminal owner TASK-007
AC-006 contributors: TASK-006; terminal owner TASK-007
AC-007 contributors: TASK-006; terminal owner TASK-007
AC-008 contributors: TASK-003, TASK-006; terminal owner TASK-007
AC-011 contributor: TASK-006 revision persistence/invariant only; terminal owner TASK-009
```

## 13. Proposed stable TEST registry

| Test ID | Required evidence |
|---|---|
| `TEST-DOMAIN-006` | aggregate/value/state/cap invariants and marker compile-fail |
| `TEST-MAPPER-006` | exact domain↔row round trips plus every malformed row class |
| `TEST-MIGRATION-006` | immutable 0001 digest/order, fresh upgrade/reopen/rollback and exact 0000 preservation |
| `TEST-SCHEMA-006` | complete sqlite_schema/table/index/FK/trigger/row allowlist and corruption negatives |
| `TEST-COMMAND-006` | exact port/digest/binding plus external-vs-pure transaction/replay/reject/recovery matrix |
| `TEST-CONCURRENCY-006` | duplicate command, shared Blob, expected revision and queue/shutdown races |
| `TEST-EVENT-006` | atomic state/outcome/events, allocator order/rollback/overflow and append-only denial |
| `TEST-CUSTODY-006` | sole-DurableBlob/single-Member proof, shared Blob/distinct Asset and no revision on Location change |
| `TEST-ERROR-006` | exact variant/code/retry/static-display mapping and canary redaction |
| `TEST-RECOVERY-006` | migration/transaction SIGKILL and statement fault prefixes with exact restart result |
| `TEST-LIFECYCLE-006` | receipt drop, panic, backpressure and joined shutdown/lock lifetime |
| `TEST-ARCH-006` | dependency/public surface/file scope and representative forbidden fixtures |
| `TEST-SUPPLY-006` | locked/offline/minimal dependency policy and fresh advisories/licenses/sources |
| `TEST-DOC-006` | proposal/ADR/requirements/AC/TEST/lifecycle/downstream ownership agreement plus negative fixtures |

Every ID maps to a non-empty argv in `scripts/verify-task-006.sh`. Developer mode
may skip only explicitly formal SIGKILL stress and supply attestation and must print
`FAST_PASS`; completion requires formal `PASS` for all fourteen IDs on the exact
committed candidate. The formal aggregate retains TASK-005 once and the separate
TASK-003 real-second-UID job remains unchanged.

## 14. Acceptance-to-test mapping

| AC | Required TEST evidence |
|---|---|
| AC-082 | DOMAIN, MAPPER, ERROR, ARCH |
| AC-083 | MIGRATION, SCHEMA, RECOVERY |
| AC-084 | DOMAIN, COMMAND, CONCURRENCY, CUSTODY |
| AC-085 | COMMAND, CONCURRENCY, RECOVERY, ERROR |
| AC-086 | DOMAIN, CONCURRENCY, EVENT, ERROR |
| AC-087 | COMMAND, EVENT, RECOVERY, SCHEMA |
| AC-088 | CUSTODY, MAPPER, ARCH, ERROR |
| AC-089 | CONCURRENCY, LIFECYCLE, RECOVERY |
| AC-090 | ARCH, SUPPLY, DOC plus retained aggregate |

Security verification at completion must explicitly report `SEC-017`, `SEC-020`
and `SEC-021`. `SEC-005`, `SEC-013` and `SEC-014` are `NOT_APPLICABLE` to a
transport-free store API except for proving no caller principal/tenant claim was
introduced; TASK-007 owns their product boundary.

## 15. Implementation order after acceptance

```text
STEP-1  Canonical synchronization + ADR-0008 + exact start record; run retained baseline.
STEP-2  Pure domain values/aggregates/errors and boundary/compile-fail tests.
STEP-3  Event and port DTO contracts; app ID/clock/service seams with fakes.
STEP-4  Verify the frozen 0001 SQL/checksum and build prefix-aware migration/schema validators.
STEP-5  Integrate migration-before-worker startup without changing bootstrap recovery.
STEP-6  Implement command ledger and typed store jobs on the existing single writer.
STEP-7  Implement managed registration, revision and Location transactions/mappers.
STEP-8  Add complete fault/concurrency/corruption/lifecycle/architecture gates.
STEP-9  Run developer then formal aggregate, review exact diff, record per-ID evidence.
STEP-10 Mark TASK-006 DONE only after reviewed committed formal CI; revoke authority.
```

No step automatically enters TASK-007.

## 16. Candidate start record

The following block may be copied into the Plan only after independent review,
canonical synchronization and an
explicit user instruction to implement:

```text
TASK006_CANONICAL_GATE: ACCEPTED
TASK006_LIFECYCLE: IN_PROGRESS
TASK006_IMPLEMENTATION_AUTHORITY: TASK_006_ONLY
TASK006_ERROR_CODES_ADDED: OPERATION_CANCELLED

SCOPE: TASK-006 ONLY — Asset domain, CommandRecord/event persistence and immutable
       0001_library_assets; no source/CAS orchestration or product transport.
FEATURES: FUNC-002; FUNC-003 (domain/persistence contribution only)
REQUIREMENTS:
  REQ-001, REQ-002, REQ-004, REQ-005, REQ-008, REQ-011, REQ-012,
  DATA-001, DATA-007, DATA-009, DATA-010, DATA-011, DATA-013,
  SEC-017, SEC-020, SEC-021, REL-001, REL-004, REL-005, REL-006
DECISIONS:
  BASE-007, BASE-008, BASE-009, BASE-011, BASE-013, BASE-014, BASE-016,
  BASE-017, BASE-018,
  DEC-003, DEC-006, DEC-007, DEC-008, DEC-016, DEC-017, DEC-018,
  DEC-019, DEC-020, DEC-021, DEC-022,
  ADR-0001, ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007, ADR-0008
PREREQUISITES: TASK-004 DONE; TASK-005 DONE
ACCEPTANCE:
  AC-082, AC-083, AC-084, AC-085, AC-086, AC-087, AC-088, AC-089, AC-090
CONTRIBUTOR_ACCEPTANCE: AC-002, AC-005, AC-006, AC-007, AC-008
TESTS:
  TEST-DOMAIN-006, TEST-MAPPER-006, TEST-MIGRATION-006, TEST-SCHEMA-006,
  TEST-COMMAND-006, TEST-CONCURRENCY-006, TEST-EVENT-006, TEST-CUSTODY-006,
  TEST-ERROR-006, TEST-RECOVERY-006, TEST-LIFECYCLE-006, TEST-ARCH-006,
  TEST-SUPPLY-006, TEST-DOC-006
DEVELOPER_GATE: scripts/verify-task-006.sh developer
FORMAL_COMPLETION_GATE: scripts/verify-task-006.sh formal
AUTHORIZED_FILES: proposal §3 exact list and restrictions
FORBIDDEN: proposal §3.1; TASK-007 and later remain unauthorized
```

## 17. Normative external behavior references

The migration and transaction contract is interpreted against the pinned SQLite
3.53.4 runtime and these upstream contracts:

- [STRICT tables](https://sqlite.org/stricttables.html): accepted storage classes,
  datatype enforcement and integrity-check behavior;
- [transactions](https://sqlite.org/lang_transaction.html): one active write
  transaction, `BEGIN IMMEDIATE`, commit and rollback behavior;
- [foreign keys](https://sqlite.org/foreignkeys.html): explicit enablement,
  composite-key requirements and child-key index expectations;
- [`RETURNING`](https://sqlite.org/lang_returning.html): top-level DML result
  semantics; and
- [PRAGMA reference](https://sqlite.org/pragma.html): `quick_check`,
  `foreign_key_check`, schema introspection and connection hardening behavior.

These references do not authorize ambient SQLite behavior. The checked-in migration,
runtime tuple, compile options and per-connection hardening remain the narrower
repository contract established by TASK-004 and this proposal.

## 18. Acceptance and blocker closure

The v0.2.1 external review accepted the corrected migration, architecture and test
contract. The user accepted that reviewed candidate and authorized TASK-006-only
execution on 2026-08-28. Canonical Specification v1.1.22, ADR-0008 and the exact
Plan start record close the former decision/traceability blockers. The retained
pre-start document, formatting, naming and TASK-005 developer gates pass. There is
no unresolved TASK-006 blocker; TASK-007 and later remain unauthorized.

## 19. Formal completion evidence

Implementation commit `60b6616c20d677632ca25b8b72340fc3a639db54` introduced the
exact §3 candidate. Reviewed run `33256714550` then exposed one `REPO_STALE`
cross-process lock-lifetime race in retained TASK-005 recovery: a concurrently
spawned process could temporarily inherit a close-on-exec duplicate of the Blob
lock descriptor and extend the kernel lock beyond the logical authority lifetime.
Commit `10455605556984e48def16efc27fb52338109944` made the private Blob lock guard
explicitly unlock on drop and added a surviving-duplicate-descriptor regression
test. It changed no public API, dependency, migration, architecture boundary or
TASK-006 behavior.

The exact correction passed a second complete local
`scripts/verify-task-006.sh formal` run. Reviewed arm64 `macos-26` GitHub Actions run
`33257331689` then passed at commit `10455605556984e48def16efc27fb52338109944`:
the TASK-006 formal aggregate completed successfully in 12m05s and the retained
TASK-003 real second-UID job completed successfully in 6m48s. All fourteen stable
TASK-006 TEST IDs, migration and transaction SIGKILL/fault evidence, locked offline
workspace gates, retained TASK-005 1/10/100 GiB O(buffer) tests, supply-chain policy
and both CI jobs passed. Required unexecuted tests: `NONE`.

AC-082 through AC-090 are `PASS`. `SEC-017`, `SEC-020` and `SEC-021` are `PASS`;
`SEC-005`, `SEC-013` and `SEC-014` are `NOT_APPLICABLE` to this transport-free
store API, and the architecture gate proves no caller-supplied principal or tenant
boundary was introduced. Final diff review found no scope creep, migration rewrite,
architecture drift, unpinned dependency, unsafe expansion, secret/debug bypass,
unbounded retry/queue or TASK-007 behavior. TASK-006 is `DONE`, its implementation
authority is `NONE`, and no later task is authorized.
