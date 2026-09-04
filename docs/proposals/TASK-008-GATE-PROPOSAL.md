---
title: "TASK-008 verify/read/materialize/observability start-gate proposal"
project: "梦夏 / MengXia"
document_role: "Draft TASK-008 pre-start implementation supplement"
status: "ACCEPTED_TASK_008_IN_PROGRESS"
version: "0.2.5"
date: "2026-09-04"
canonical_specification_reviewed: "IMPLEMENTATION_SPEC.md v1.1.29"
repository_head_reviewed: "4fabe064825d79706740811e341698121c4e96d6"
---

# TASK-008 Gate Proposal

## 0. Gate verdict

The completed TASK-001 through TASK-007 foundation is sufficient to design
TASK-008, but it does not authorize TASK-008 production code. This proposal closes
the operation, pagination, destination authority, restart recovery, verification,
observability, health and finite-resource contracts required before implementation.

```text
TASK008_CANONICAL_GATE: ACCEPTED
TASK008_LIFECYCLE: IN_PROGRESS
TASK008_IMPLEMENTATION_AUTHORITY: TASK_008_ONLY
TASK008_PROPOSAL_VERSION: 0.2.5
TASK008_INDEPENDENT_REVIEW: PASS_2026_09_04
TASK008_UNRESOLVED_BLOCKING_FINDINGS: NONE
```

The accepted v0.2.5 contract retains the v0.2.4 materialize replay, durable startup
classification, SQLite cancellation, closed-observability and issue-schema
resolutions; replaces the impractical per-Member global Location scan with an
existing-index current-Blob keyset; closes the protocol build-script scope, dense
ListAssets pagination, exact creation-event predicate, operation-specific recovery
and canonical metric-label findings; preserves TASK-007's exact protocol 1.1
evidence through stable fixtures before the current descriptor advances to 1.2;
closes the event-allocator/snapshot and no-progress corruption cases; and records
query-plan evidence from the bundled SQLite 3.53.4 runtime. Independent review on
2026-09-04 found no unresolved blocker, and the user conditionally authorized
execution after that review. Canonical synchronization and ADR-0011 activate only
the exact `TASK_008_ONLY` scope below. No text in this accepted supplement authorizes
a migration, destructive orphan cleanup, storage-root rebind, Admin authority,
Provider/Plugin behavior or TASK-009+ work.

## 1. Inputs and repository evidence

This candidate was derived in repository authority order from:

1. `docs/spec/IMPLEMENTATION_SPEC.md` v1.1.29;
2. `docs/spec/DECISIONS.md` v0.3.30 and accepted ADR-0001 through ADR-0010;
3. `docs/spec/IMPLEMENTATION_REVIEW.md` v1.1.40;
4. `docs/spec/IMPLEMENTATION_PLAN.md` v0.3.40;
5. `docs/spec/PROJECT_INTAKE_REPORT.md` v1.3.35;
6. accepted TASK-003 through TASK-007 supplements and their completed code;
7. reviewed formal CI run `33482363576` at commit `7c361399211d4551f16b1397195d7ad6f7e05479`;
8. current clean canonical head `4fabe064825d79706740811e341698121c4e96d6`
   plus a fresh local `scripts/verify-repository.sh docs` pass. No unarchived CI run
   number is treated as repository authority; later reviewed evidence must first be
   synchronized into Intake/Plan before completion depends on it.

| Prerequisite | Repository evidence | Result |
|---|---|---|
| TASK-003 authenticated IPC | protected UDS, server-derived owner UID, bounded protocol 1.0/1.1 lifecycle | PASS |
| TASK-004 Library ownership/store | opaque owner, fixed SQLite, exact schema validation, bounded writer/read workers | PASS |
| TASK-005 local CAS | opaque root/source, bounded streaming, no-clobber durable promotion, orphan observation | PASS |
| TASK-006 Asset persistence | immutable `0001_library_assets`, graph/CommandRecord/events and read connections | PASS |
| TASK-007 product composition | copy-ingest protocol 1.1, durable claim/CAS/registration and fatal runtime gate | PASS |
| Asset query ports | no bounded Inspect/List port exists | EXPECTED_GAP owned here |
| Managed-Blob read/materialize | no internal read/materialize port or destination authority exists | EXPECTED_GAP owned here |
| Verification report | startup validates SQLite but has no product report/deep-CAS API | EXPECTED_GAP owned here |
| Core observability | ad-hoc stderr status only; no typed log/metric/health schema | EXPECTED_GAP owned here |
| Current implementation authority | every current-state document says `NONE` | PASS / proposal work only |

No repository discrepancy requires a migration or architecture reversal. The
absence of TASK-008 interfaces is the expected work of this task, not a blocker to
draft review.

## 2. Blocking gaps and exact proposed resolutions

### 2.1 Query ordering cannot use UUIDv7 or mutable Asset fields

Classification: `ARCHITECTURE / DATA_INTEGRITY`

UUIDv7 generation is time-oriented but is not the Library commit order, and Asset
lifecycle/revision may change while a client paginates. `ListAssets` therefore uses
the immutable `domain_events.commit_sequence` of the Asset's creation event as its
membership and ordering authority. The first page captures
`event_commit_sequence.last_sequence` in the same read transaction that seeks the
maximum committed DomainEvent sequence. The adapter requires the singleton to equal
`COALESCE(MAX(domain_events.commit_sequence), 0)` before it emits any item; either
direction of mismatch is `STORAGE_CORRUPTION`. The maximum is obtained by the unique
commit-sequence index's endpoint plan rather than a table scan. Later pages include
only Asset creation events at or below that snapshot. Cursor format 1 recognizes
exactly
`schema_version=1 AND event_type='asset.registered.v1' AND
aggregate_kind='ASSET'` as its creation-event predicate. Every qualifying row must
have a 16-byte aggregate ID, exactly one matching canonical Asset, and a matching
`COMPLETED` CommandRecord whose `result_kind='ASSET'` and `result_id` equals that
aggregate ID; every Asset in the captured membership must have exactly one such
creation event. Any future operation that creates an Asset must reuse this exact
event contract or first accept a new closed creation-event registry and its cursor
compatibility rule; it cannot silently broaden the predicate.

Each continuation requires `0 < last_examined_sequence < captured_last_sequence <=
9223372036854775807`, requires the current allocator to be at least the captured
snapshot, and proves by unique-index point lookup that DomainEvents still exist at
both the last-examined and captured snapshot sequences. A mismatch is
`STORAGE_CORRUPTION`; malformed ordering or an
out-of-range cursor is `VALIDATION_ERROR`. Each request then steps the unique
`domain_events.commit_sequence` index in ascending
order after the cursor's last examined sequence and stops immediately at whichever
comes first: the `STORE_SCAN_PAGE_MAX=256`th actually stepped event, the event that
produces the requested `page_size`th qualifying Asset, or snapshot exhaustion. The
response includes every qualifying Asset stepped before that stop; it never scans
past a qualifying row that it cannot return. The next cursor records the actual
last stepped event, not a prefetched/window-end row. A request may legally return
fewer than the requested size, including an empty page, with a next cursor when
unexamined snapshot events remain. This makes rows examined rather than only rows
returned the hard SQL-work bound; no event-type index or migration is required. If
the bounded seek returns no row while the last examined sequence is below the
captured snapshot, it returns `STORAGE_CORRUPTION`; it never fabricates a stepped
sequence, repeats the same cursor or creates an infinite empty-page chain. Because
migration 0001 allocates and inserts each DomainEvent in the same transaction, every
stepped sequence must equal the preceding stepped/cursor sequence plus one. An
interior gap is `STORAGE_CORRUPTION`, not a silently skipped event.

Returned mutable fields are current at each page read and carry the Asset revision.
Concurrent mutation cannot add or remove membership from the captured snapshot. A
missing canonical Asset row is corruption, not a silently skipped deletion. The
cursor advances by the last examined unique commit sequence; Asset ID is not a
fictional tiebreaker.

This window is required by repository reality: migration 0001 has no
`(event_type, aggregate_kind, commit_sequence)` index. A filtered List query either
uses `domain_events_aggregate_idx` plus `USE TEMP B-TREE FOR ORDER BY` or scans the
unique commit-sequence index while applying an unbounded residual filter. The
proposal must not claim `page_size + 1` examined rows. Adding an index/migration is
expressly rejected for TASK-008; query-plan regressions freeze the bounded-window
alternative plus the allocator/max endpoint and snapshot-existence point lookups.
The exact auxiliary SQL shapes are:

```sql
SELECT last_sequence
FROM event_commit_sequence
WHERE singleton = 1;

SELECT COALESCE(MAX(commit_sequence), 0)
FROM domain_events;

SELECT 1
FROM domain_events
WHERE commit_sequence = ?;
```

The maximum and point probes must use the covering unique commit-sequence index and
must not report a table scan or temporary B-tree. Continuations execute the point
shape for both last-examined and snapshot values; one query may bind both values if
its plan and two-result cardinality remain exact and bounded.

TASK-008 has no lifecycle filter because filtering by mutable lifecycle would need
historical projections not present in immutable migration 0001. Adding such a
filter without a migration would produce duplicates or omissions. Future tasks may
add a versioned filter only with an accepted snapshot design.

### 2.2 InspectAsset needs a bounded graph shape

Classification: `SPECIFICATION / RESOURCE_BOUND`

One AssetRevision can contain up to 64 parents, 64 Representations, 64 Resources
per Representation and 4096 Members per Resource. Each Member's Blob may also have
more than one Location: migration 0001 and the accepted TASK-006
`record_location` contract deliberately allow another Location to be added. Returning
the graph as one nested message is therefore unbounded relative to the frame cap, and
collapsing Blob -> Location to an arbitrary single row would not return the canonical
graph. `InspectAsset` selects exactly one immutable revision and returns:

- one bounded Asset/revision header;
- at most 64 parent revision IDs in the header;
- a page of flattened `AssetMemberView` projections ordered by
  `(representation_id, resource_id, member_ordinal, backend_id, location_id)`;
- repeated Representation/Resource/Member metadata in every projection;
- one projection for every Location belonging to the Member Blob at that Member's
  captured Blob revision, without `backend_id`, `locator` or a filesystem path;
- exactly one absent-Location sentinel projection for a valid `UNMANAGED` Member
  whose Blob has no qualifying Location; a `MANAGED` Member with none is corruption.

Every accepted revision has non-empty Representations, Resources and Members under
the TASK-006 domain contract, so this flat form does not lose an empty node. An
absent revision selector means “latest at first page”; the selected revision ID and
Asset revision are frozen into the cursor. When scanning one Member, the adapter
captures its Blob revision and carries it in the cursor. The accepted TASK-006
`record_location` transaction increments that Blob revision atomically with every
Location addition; a continuation therefore returns `CONFLICT` if membership changed
instead of silently duplicating/omitting a Location. A Member not yet reached uses
its current Blob revision when its enumeration begins. Location lifecycle/revision fields
are current at each bounded page read and do not change membership. If the Asset
revision or current Member Blob revision changes before a later page, the server
returns `CONFLICT` and the caller starts a fresh inspection; it never mixes one
Member's Location membership across Blob revisions.

The adapter must not implement this ordering as one multi-table join plus global
sort. It walks existing indexes hierarchically: seek the next Representation under
the selected revision, the next Resource under that Representation, and then the
next Member ordinal. For the current Member it uses migration 0001's existing
`locations(blob_digest, backend_id, location_id)` index and orders only that Blob's
Locations by `(backend_id, location_id)`. The first Location seek is exactly the
bounded shape `WHERE blob_digest=? ORDER BY backend_id, location_id LIMIT ?`. A
continuation first resolves the cursor's last Location ID through the Location
primary key, requires that row to name the current Blob, and then uses the exact
row-value keyset shape `WHERE blob_digest=? AND (backend_id, location_id) > (?, ?)
ORDER BY backend_id, location_id LIMIT ?`. The recovered backend ID is transient
adapter state inside the same bounded read transaction; it never enters the cursor,
response, CLI output or log. A missing/mismatched cursor Location is
`VALIDATION_ERROR` after the captured Asset and Blob revisions have been revalidated.

Across all Members visited by one request, Location queries return at most
`INSPECT_LOCATION_ROWS_PER_PAGE_MAX=65` rows: no more than the 64 emitted projections
plus one final lookahead. Exhausted zero-Location queries contribute no returned
row, and only the final non-exhausted Member can contribute lookahead. The adapter
returns at most `page_size` projections and performs at most
`INSPECT_SELECTS_PER_PAGE_MAX=512` SELECTs including header/parent validation,
cursor-key recovery, hierarchy seeks, bounded Location/Blob-revision validation and
lookahead. Query-plan tests reject `USE TEMP B-TREE`, a scan not constrained by the
current `blob_digest`, or failure to use the Location primary key for cursor-key
recovery and `locations_blob_backend_idx` for both Location seeks. They prove the
row/select caps with maximum accepted graphs and dense/sparse multi-backend
multi-Location fixtures. The adapter must not narrow enumeration to the current
runtime backend: TASK-006 permits other backend identities and multiple Locations
for one Blob, and omitting them would no longer return the canonical graph. It also
must not combine `blob_digest=?` with `ORDER BY location_id`; migration 0001 makes
that shape choose `locations_blob_backend_idx` plus a temporary sort. Neither
`INDEXED BY` pinning nor a whole-Library fallback is an accepted repair.

The accepted query schedule reserves at most 32 SELECTs for header/parent/cursor
validation and at most seven for each of `page_size + 1` projection/advance slots,
so the 64-row maximum is bounded by
`32 + 7 * 65 = 487 < 512`; an implementation needing another query must stop for
review rather than weaken the cap. Exhausting either invariant is `INTERNAL_ERROR`,
never an unbounded fallback. The legal worst case includes 16,777,216 Members and an
independently unbounded Location count for a Blob; V1 handles it through bounded
multi-request traversal without rejecting a graph TASK-006 accepts, exposing a
backend ID or widening the frame cap.

### 2.3 Verification must distinguish metadata checks from byte hashing

Classification: `RELIABILITY / PERFORMANCE`

Startup retains the existing SQLite version/schema/checksum/row/foreign-key/
`quick_check` gates and bounded staging observation. It never hashes all Blob bytes
and never enumerates the entire CAS before endpoint publication.

Explicit `VerifyLibrary(NORMAL)` checks canonical database invariants, command/event
relationships, registered Blob/Location descriptors, exact CAS namespace entries,
file authority/type/length and staging observations without reading Blob bodies.
Explicit `VerifyLibrary(DEEP)` performs the same checks and streams every selected
managed Blob through SHA-256. Deep verification is opt-in, one run at a time,
deadline/cancellation controlled and O(configured buffer) memory. Neither mode
updates `verified_at`, Location lifecycle, events or any canonical row.

### 2.4 A report must remain bounded without inventing verification persistence

Classification: `ARCHITECTURE / RESOURCE_BOUND`

TASK-008 maintains at most four completed in-process verification reports. A report
contains one generated verification ID, mode, captured event sequence, bounded
summary counters and at most 4096 safe typed issues. Additional issues increment a
`dropped_issue_count`; they do not allocate memory. Completion atomically inserts
the report and evicts the oldest completed report if necessary. A currently listed
report is held by `Arc` until that request finishes.

Reports are diagnostic observations, not canonical domain state. They disappear on
daemon restart. `ListIntegrityIssues` returns `NOT_FOUND` after restart or FIFO
eviction. This is its documented cursor expiry policy. A cancelled/deadline-failed
verification publishes no partial report and returns the typed terminal error.
Verification IDs are transport/report-scoped diagnostic UUIDs, not canonical Core
object IDs or authority; their non-persistence cannot satisfy or weaken canonical
object identity rules. They use the accepted UUIDv7 generator and at most eight collision
attempts against the four resident IDs; clock/entropy failure or exhausted attempts
returns `ID_GENERATION_UNAVAILABLE` before a scan starts.

### 2.5 Orphan reconciliation is diagnostic, not destructive

Classification: `SECURITY / OWNERSHIP`

TASK-005 staging orphans remain preserved and charged. TASK-008 may classify and
report a prior-process staging orphan, a registered managed Blob with no safe file,
an unregistered canonical Blob, an unsafe namespace entry or a changed backend. It
must not unlink, move, adopt, register, rebind or rewrite any such object. TASK-022
alone may later delete bytes after its Admin/destructive gate and OQ-008.

Here “reconciliation” means bounded comparison of durable metadata with observed
custody and an operator-visible typed issue, not automatic repair. A changed local
backend starts the endpoint in explicit read-only custody-degraded mode so status,
metadata queries and verification remain available; ingest and materialize stay
closed and no Location is rewritten. SQLite/schema/owner/lock corruption remains a
fatal local invariant and does not become degraded availability.

### 2.6 MaterializeAsset cannot use an unrecorded best-effort copy

Classification: `RELIABILITY / DATA_INTEGRITY`

`MaterializeAsset` is an external filesystem effect and therefore uses caller-owned
`command_id`, a version-frozen request digest and the existing migration 0001
CommandRecord. It selects exactly one Member through an
AssetRevision/Representation/Resource chain and writes one
caller-selected final file. The completed row uses existing
`result_kind='ASSET_REVISION'` and that selected revision ID. The request digest
binds the Asset, revision, Representation, Resource, ordinal and exact destination
selector. The stored row plus an exact revalidated request is sufficient for
deterministic replay without adding a result kind or destination column; the row
alone is deliberately not a queryable materialization-history record.

This deliberately does not consume migration number 0002 or modify migration 0001.
TASK-009 retains ownership of its extensible outcome/migration gate. A completed
replay returns the original result and never recreates a final file that the user
later removes or changes.

### 2.7 External destination recovery needs evidence local to that destination

Classification: `SECURITY / CRASH_RECOVERY`

After the durable CommandRecord claim, the materializer creates a versioned,
checksum-protected intent and fixed command-scoped staging file in the retained
destination parent. The intent is durably synchronized before staging. It binds the
Library, command, request digest, selected graph IDs, Blob digest/length, parent
device/inode and exact final basename. A valid intent is the only authority to
clean or resume a staging file. An invalid/unknown intent or conflicting name fails
closed and is never automatically deleted.

The final target is published no-clobber in the same directory and synchronized
before the CommandRecord becomes completed. After publish, cancellation cannot
override completion. Exact retry recovers only the same binding and same retained
parent/final identity; it never scans arbitrary directories or retries every old
claim.

### 2.8 Core observability must not wait for Plugin/Admin work

Classification: `CONFLICT / OBSERVABILITY`

TASK-008 supplies a dependency-neutral typed Core log event, a closed metric
catalog with bounded labels and a health model. The daemon owns a bounded
non-blocking log sink and fixed-format encoder. TASK-013 later extends the schema
for Plugin/Broker/audit and retains SEC-008/SEC-019 policy ownership; it does not
replace the Core baseline.

No raw path, locator, source/destination bytes, logical name, media metadata,
credential-like value, SQL text, errno, ACL principal or arbitrary error string can
enter the typed log event. Metrics accept only closed enums; request, command,
Asset, Run and correlation IDs are forbidden labels.

This baseline must refine rather than replace canonical §14.1 and §15.1–§15.2:
the encoder accounts for every applicable structured field, the metric catalog
retains every stable per-error metric name, and it adds bounded duration/latency,
queue/WAL and storage/integrity observations. “No exporter” and “no numeric SLO” do
not waive those metrics. Exact field applicability, histogram buckets, counter
overflow, `MENGXIA_LOG_LEVEL` filtering and later Plugin/Provider extension points
are fixed in §12–§13 and ADR-0011.

### 2.9 The canonical materialize row is not an exact member selector

Classification: `SPEC_STALE / API_CONTRACT`

Specification §10.2 names canonical operation ID `asset.materialize.v1` and sketches
`AssetRevision, Representation, target`, but one Representation may contain several
Resources and each Resource may contain several Members. That sketch cannot select
one output file deterministically. TASK-008 must retain the canonical operation ID
and refine the request to bind Asset, AssetRevision, Representation, Resource and
member ordinal. The store validates the complete containment chain; none may be
inferred from a non-unique display value.

The same row's “materialization record” is a semantic exact-request response/replay
view whose identity is the existing CommandRecord `command_id`; it is not authority
to add a new table or result kind. The response contains that command ID and the exact
selected immutable IDs/digest/length. Representation, Resource, ordinal and
destination selector are reconstructed only from the exact request after its
versioned digest and graph are revalidated; they are not independently recoverable
from the ledger by command ID. TASK-008 therefore provides no
`GetMaterialization(command_id)` query. Canonical acceptance must replace the
ambiguous “materialization record” wording in §10.2 with this exact replay-result
contract before implementation starts; any independently queryable history needs a
later migration gate.

### 2.10 The delivered external-ingest claim/replay path cannot recover materialize

Classification: `CONFLICT / RELIABILITY`

The completed TASK-006/007 `claim_external_ingest` path converts a prior-runtime
`CLAIMED` row to `RECOVERY_REQUIRED`, never reacquires that row, and replays an
`ASSET_REVISION` by requiring `asset.revision.created.v1`. Those accepted ingest and
revision semantics remain unchanged. Materialize therefore receives separate
provider-neutral claim, observe, compare-and-swap reacquire, complete, disposition
and replay ports; it must not add a `Recover` variant to
`ExternalClaimOutcome`, reuse `ExternalClaimGuard`, or change the shared
`replay_result` dispatch.

Materialize recovery is two phase. First, an observation returns the exact durable
state without mutating it. The app revalidates the authenticated binding, current
destination authority and the §11 intent/final state. Only a recoverable state may
call the materialize-specific reacquire transaction. That transaction matches
command ID, principal, operation ID, request digest, prior state/runtime and the
expected safe error, then atomically writes the current runtime ID, sets
`state='CLAIMED'` and clears `safe_error_code`. The immutable 0001 CHECK therefore
remains satisfied. A competing caller that loses the compare-and-swap receives
current-runtime in-progress and performs no filesystem action. This physical
reacquire classifier is distinct from the startup ledger classifier in §4.1: startup
may durably move a prior-runtime `CLAIMED` row to `RECOVERY_REQUIRED`, but it cannot
authorize filesystem recovery without the exact resubmitted destination request.

For `asset.materialize.v1`, the only valid `RECOVERY_REQUIRED` safe codes are
`STORAGE_CONFIGURATION_ERROR`, `STORAGE_IO_ERROR`,
`ID_GENERATION_UNAVAILABLE` and `INTERNAL_ERROR`, matching the existing external
recovery family without changing ingest. Any other code/state/result combination is
`STORAGE_CORRUPTION`. A safe code being in this set is necessary but never sufficient
for reacquire; the exact physical classifier remains mandatory.

The materialize replay mapper dispatches on both
`operation_id='asset.materialize.v1'` and
`result_kind='ASSET_REVISION'`, then revalidates the exact request and immutable
graph. Existing AssetRevision replay remains event-backed and byte-for-byte
unchanged. Cross-operation/result tests must prove neither mapper can consume the
other's row. An ingest-origin `RECOVERY_REQUIRED` issue uses
`OPERATOR_OR_RUNTIME_ACTION`; `RETRY_EXACT_COMMAND` is reserved for a materialize
state that passed the explicit recovery classifier.

### 2.11 Canonical readiness requires a two-stage publication contract

Classification: `CONFLICT / OPERABILITY`

Specification §15.4 requires the Library lock, valid schema, operational writer and
completed mutation recovery before `READY`. Binding the authenticated socket is not
itself readiness. TASK-008 may bind after local authority is established, but it
must initially publish `NOT_READY`, expose only bounded status, and keep all product
mutation admission closed. It transitions to `READY` only after the writer is
operational and every startup local mutation at the captured boundary has either
completed or reached a durably classified terminal/recovery state. An in-memory
count of an old `CLAIMED` row is not durable classification and cannot satisfy this
gate.

“Recovery complete” does not mean blindly finishing every historical external
effect at startup. A valid `RECOVERY_REQUIRED` row is a completed classification:
the exact command/capability remains restricted while unrelated safe capabilities
may become ready. Provider reconciliation remains separately bounded and may
continue after readiness as `DEGRADED_DEPENDENCY`. Canonical §13.5/§15.4 must adopt
this two-stage and classified-recovery refinement before implementation.

## 3. Exact implementation scope after acceptance

Only these paths may change after an accepted canonical start record:

```text
proto/core/v1/handshake.proto
proto/core/v1/handshake.pb
proto/core/v1/handshake.provenance
crates/mengxia-core-proto/build.rs
crates/mengxia-core-proto/src/lib.rs
crates/mengxia-core-proto/src/session.rs
crates/mengxia-ports/src/lib.rs
crates/mengxia-app/src/lib.rs
crates/mengxia-app/src/config.rs
crates/mengxia-app/src/asset_persistence.rs      # materialize-specific guard/clock seam
crates/mengxia-app/src/ingest.rs               # active-custody observation only
crates/mengxia-app/src/asset_query.rs            # new
crates/mengxia-app/src/verification.rs           # new
crates/mengxia-app/src/materialize.rs            # new
crates/mengxia-app/src/observability.rs          # new
crates/mengxia-platform-fs/src/lib.rs
crates/mengxia-platform-fs/src/blob_storage.rs
crates/mengxia-platform-fs/src/materialization.rs # new; no new unsafe/FFI
crates/mengxia-storage-local/src/lib.rs
crates/mengxia-storage-local/tests/task_008_read_materialize.rs # new
crates/mengxia-store-sqlite/src/lib.rs
crates/mengxia-store-sqlite/src/lifecycle.rs
crates/mengxia-store-sqlite/src/asset_repository.rs
crates/mengxia-store-sqlite/src/asset_query.rs    # new
crates/mengxia-store-sqlite/src/verification.rs   # new
crates/mengxia-store-sqlite/tests/task_008_queries.rs # new
bins/mengxiad/src/main.rs
bins/mengxia/src/main.rs
crates/mengxia-testkit/tests/task_008_foundation.rs
crates/mengxia-testkit/tests/task_007_foundation.rs # retained protocol 1.1 evidence only
crates/mengxia-testkit/tests/fixtures/task_007/handshake-v1.1.proto      # new
crates/mengxia-testkit/tests/fixtures/task_007/handshake-v1.1.pb         # new
crates/mengxia-testkit/tests/fixtures/task_007/handshake-v1.1.provenance # new
crates/mengxia-testkit/tests/ci_orchestration.rs
crates/mengxia-testkit/tests/document_traceability.rs
crates/mengxia-testkit/tests/naming.rs
scripts/verify-task-008.sh
scripts/verify-repository.sh
.github/workflows/ci.yml                         # gate title/mapping only; retained jobs unchanged
docs/proposals/TASK-008-GATE-PROPOSAL.md
docs/spec/IMPLEMENTATION_SPEC.md
docs/spec/DECISIONS.md
docs/spec/IMPLEMENTATION_REVIEW.md
docs/spec/IMPLEMENTATION_PLAN.md
docs/spec/PROJECT_INTAKE_REPORT.md
docs/spec/adr/ADR-0011-task-008-read-and-materialization-boundary.md
AGENTS.md
```

Only the named new source/test files are allowed; this is not a wildcard grant for
their parent directories. Root/crate Cargo manifests and `Cargo.lock` remain
byte-identical. No new third-party dependency or package is authorized.

Before changing the current protocol artifact, STEP-2 copies the accepted TASK-007
protocol 1.1 source, descriptor and provenance into the three named immutable test
fixtures and pins their current reviewed hashes. `task_007_foundation.rs` is changed
only to make `TEST-PROTO-007` verify that exact 1.1 fixture plus backward-compatible
1.1 clauses in the current schema. It must not update the TASK-007 expected hashes
to the new 1.2 artifact. `TEST-PROTO-008` separately owns the exact current 1.2
source/descriptor/provenance hashes. No TASK-007 product source or behavior changes.

`crates/mengxia-platform-fs/src/macos_ffi.rs` is intentionally not authorized. The
new sibling module can consume the existing parent-scoped ACL inspection through
the crate's current safe policy helpers; no ABI/visibility/unsafe change is required.
If implementation proves otherwise, TASK-008 stops for a new reviewed scope change
rather than silently editing the audited FFI boundary.

### 3.1 Explicitly forbidden scope

- any edit to `migrations/sqlite/0000_store_bootstrap.sql` or
  `migrations/sqlite/0001_library_assets.sql`;
- migration 0002 or any schema/table/column/index/trigger change;
- mutation of Asset lifecycle, revision graph, Location lifecycle or `verified_at`;
- automatic orphan/canonical Blob deletion, adoption, movement or registration;
- storage-root rebind or locator/backend rewrite;
- overwrite, merge, recursive directory materialization or multi-file atomicity;
- CAS root, backend locator, SQLite path or descriptor exposure through proto/CLI;
- Admin, TCP/HTTP, Project, Provider, Plugin, Credential, Rights, GC or Purge;
- SecurityDoctor Admin operation, SecurityAuditEvent or SEC-008 policy logic;
- generic CRUD/query SQL, raw row/connection handles or arbitrary metric labels;
- logging of raw request fields, path bytes, content metadata or dynamic errors;
- detached workers, infinite waits, unbounded reports/pages/queues or blind retries;
- unsafe/FFI expansion, new dependency, large refactor or TASK-009+ implementation.

## 4. Runtime architecture and ownership

The daemon continues to own one `OpenedLibrary`, one `LocalBlobStorage`, one
application service set and one endpoint. TASK-008 adds narrow capabilities:

```text
authenticated protocol 1.2 session
  -> daemon transport adapter
  -> TASK-008 application query/verification/materialize services
  -> provider-neutral read/materialize/health ports
  -> SQLite read workers or writer for materialize command disposition
  -> local CAS/destination platform authority
```

The CLI remains a framing/proto client and never imports domain, ports, SQLite,
storage-local or platform-fs. App code imports domain/ports/types only and never
imports Tokio, prost, rusqlite or platform-fs. The store never opens arbitrary user
destinations. Platform-fs never imports app/domain/store/SQLite. Storage-local is
the only adapter allowed to combine opaque CAS authority with the destination
authority.

`OpenedLibrary` may mint read/query handles and the already authorized Blob-root
capability while retaining the owner lock. No handle can outlive the opened owner.
Every store/storage/report/log worker is explicitly closed and joined before Blob
storage, SQLite workers and the Library lock are released in that order.

### 4.1 Exact startup sequence

```text
S0 capture/validate all typed configuration; mutate nothing on failure; after full
   validation, start the bounded observability sink and emit SERVICE_STARTING
S1 acquire/revalidate durable Library authority and exclusive lock
S2 open/recover SQLite and run pinned runtime/schema/checksum/row/FK/quick checks,
   including the bounded event-allocator/maximum equality check
S3 open/revalidate Blob authority and perform TASK-005 bounded staging observation
S4 compare current local backend with managed Location metadata without rewrite
S5 construct read/query, verification-report, health and allowed
   mutation services with NOT_READY and every product mutation gate closed
S6 bind the authenticated endpoint; serve bounded status only while NOT_READY
S7 run one joined, deadline-bounded local mutation ledger classifier at a captured
   command boundary; prove the writer operational; in keyset writer batches,
   validate existing recovery rows and CAS every matching prior-runtime CLAIMED row
   to RECOVERY_REQUIRED before counting it classified
S8 atomically publish READY plus exact capability bits; then start only separately
   bounded Provider/future-dependency observation
```

S0..S5 are synchronous gates but never read all Blob bodies or enumerate all
canonical Blobs. A failure in S1/S2 is fatal and the endpoint is absent. A proven
backend mismatch at S4 constructs read-only custody state rather than an ingest or
materialize service. Staging orphans remain charged under TASK-005; they do not by
themselves invent a fatal state or authorize cleanup.

The S0 sink owns no Library/store/path authority and accepts only closed events; this
allows S1/S2 startup failures and DB observations to use the same schema. Every
failure after sink construction closes and joins it. If configuration validation
itself fails before construction, the process may emit only the existing static
pre-logger stderr line and exits without claiming structured observability.

The S7 classifier captures the maximum command ID visible before product mutation
admission and performs two non-overlapping `commands(state, command_id)` passes in
writer batches of at most 256: first `state='CLAIMED'` in Command-ID keyset order,
then `state='RECOVERY_REQUIRED'` in a fresh Command-ID keyset order, both bounded by
the captured maximum. This prevents a row moved between indexed state partitions
from being skipped or treated twice as an unresolved claim. For each row it validates
operation/state/result/error/runtime invariants. An already valid
`RECOVERY_REQUIRED` row remains unchanged. A prior-runtime `CLAIMED` row for an
accepted external-effect operation is changed by compare-and-swap to
`RECOVERY_REQUIRED` with the existing safe code
`STORAGE_CONFIGURATION_ERROR`, the current trusted classification timestamp and no
result fields. This is the same conservative durable classification already accepted
for prior-runtime ingest claims; it is not proof that physical recovery is safe.
Materialize intent/final inspection still requires the exact authenticated request
and destination authority under §2.10/§11. Pure operations may not have a persisted
`CLAIMED` row; one is corruption.

S7 never scans arbitrary destination directories, scans/hashes Blob bodies, deletes
anything or waits for a Provider. At most one classifier exists. It uses the writer
only for short bounded transactions, releases it between batches and uses the §7.5
SQLite interruption contract. Until all rows at the captured boundary are durably
classified, status remains `NOT_READY` and every non-status product operation returns
the static readiness result without admission. Its fixed monotonic deadline is
`STARTUP_LOCAL_CLASSIFICATION_TIMEOUT_MS=300000`. Deadline, backpressure, clock
failure or an unclassifiable row commits no partial current batch, sets
`LOCAL_RECOVERY_OPERATOR_ACTION`, leaves the endpoint status-only and performs no
automatic retry. Previously committed batches remain valid; an operator-controlled
daemon restart resumes from durable state and a new captured boundary. This is a
finite recovery path, not an infinite startup loop or a readiness bypass.

After successful S7 completion, a classified `RECOVERY_REQUIRED` row restricts its
exact command but is not an unknown mutation, so unrelated capabilities may become
ready. The worker is owned by the daemon JoinSet and joined on shutdown. After S8, a
test-only optional-dependency seam proves that a future unavailable Provider changes
only `DEGRADED_DEPENDENCY`; no Provider adapter or production reconciliation is
introduced in TASK-008.

## 5. Protocol 1.2 and complete operation registry

Protocol major remains 1. Protocol minors 0 and 1 retain their exact TASK-003 and
TASK-007 behavior. TASK-008 adds minor 2. A TASK-008 client requests exact
`min_protocol_minor=max_protocol_minor=2`; it never accepts downgrade to a server
that cannot implement the requested operation. Existing handshake and ingest CLI
commands retain exact minor 0 and 1 requests respectively.

Canonical §10.2/§10.2.1 must freeze these six IDs and refined contracts before the
descriptor is accepted:

| Stable operation ID | Semantic operation | Canonical refinement |
|---|---|---|
| `library.status.v1` | `GetLibraryStatus` | typed liveness/readiness/availability/security/custody observation; no side effect |
| `library.verify.v1` | `VerifyLibrary` | the authenticated V1 Library owner Client may request NORMAL or explicit DEEP; no Admin listener or Admin claim is implied |
| `library.integrity-issues.list.v1` | `ListIntegrityIssues` | in-memory verification-bound bounded query with restart/eviction expiry |
| `asset.inspect.v1` | `InspectAsset` | the canonical graph is transported as a revision-frozen header plus bounded snapshot member-Location projection pages |
| `asset.list.v1` | `ListAssets` | bounded event-window snapshot query with no mutable lifecycle filter |
| `asset.materialize.v1` | `MaterializeAsset` | exact Member selector and CommandRecord-backed exact-request replay result per §2.9/§9 |

The five query IDs are API/log/metric identities and never create a CommandRecord.
Only `asset.materialize.v1` is a durable command operation ID. “Client/Admin based
on depth” in the existing verify sketch is refined for current V1: the authenticated
durable Library owner may request DEEP, while future Admin-only repair/destructive
depths remain absent and unauthorized.

The `CoreRequest.operation` oneof adds tags 2 through 7; ingest remains tag 1 and
request tags 8 through 15 are explicitly reserved. `CoreResponse.response` uses the
same operation tags, explicitly reserves tags 8 through 14 and retains error tag
15. Authority-bearing names remain reserved. Unknown/absent
oneofs return `VALIDATION_ERROR`. One authenticated connection still carries
exactly one operation and one terminal response.

| Tag | `CoreRequest.operation` field | `CoreResponse.response` field |
|---:|---|---|
| 1 | `ingest_asset_copy` | `ingest_asset_copy` |
| 2 | `get_library_status` | `get_library_status` |
| 3 | `verify_library` | `verify_library` |
| 4 | `list_integrity_issues` | `list_integrity_issues` |
| 5 | `inspect_asset` | `inspect_asset` |
| 6 | `list_assets` | `list_assets` |
| 7 | `materialize_asset` | `materialize_asset` |
| 15 | not valid in request | `error` |

```proto
// Shape-level proposal; the accepted implementation must freeze exact generated
// descriptor bytes and provenance before code is marked complete.
message GetLibraryStatusRequest { reserved 1 to 15; }

enum VerificationMode {
  VERIFICATION_MODE_UNSPECIFIED = 0;
  VERIFICATION_MODE_NORMAL = 1;
  VERIFICATION_MODE_DEEP = 2;
}

message VerifyLibraryRequest {
  VerificationMode mode = 1;
  uint64 operation_timeout_ms = 2;
  reserved 3 to 15;
}

message ListIntegrityIssuesRequest {
  string verification_id = 1;
  uint32 page_size = 2;
  bytes cursor = 3;
  uint64 operation_timeout_ms = 4;
  reserved 5 to 15;
}

message InspectAssetRequest {
  string asset_id = 1;
  optional string asset_revision_id = 2;
  uint32 page_size = 3;
  bytes cursor = 4;
  uint64 operation_timeout_ms = 5;
  reserved 6 to 15;
}

message ListAssetsRequest {
  uint32 page_size = 1;
  bytes cursor = 2;
  uint64 operation_timeout_ms = 3;
  reserved 4 to 15;
}

message MaterializeAssetRequest {
  string command_id = 1;
  string asset_id = 2;
  string asset_revision_id = 3;
  string representation_id = 4;
  string resource_id = 5;
  uint32 member_ordinal = 6;
  bytes destination_path = 7;
  uint64 operation_timeout_ms = 8;
  reserved 9 to 31;
  reserved "actor", "principal", "project_id", "backend_id", "locator",
           "cas_root", "overwrite", "recursive";
}

message CoreRequest {
  oneof operation {
    IngestAssetCopyRequest ingest_asset_copy = 1;
    GetLibraryStatusRequest get_library_status = 2;
    VerifyLibraryRequest verify_library = 3;
    ListIntegrityIssuesRequest list_integrity_issues = 4;
    InspectAssetRequest inspect_asset = 5;
    ListAssetsRequest list_assets = 6;
    MaterializeAssetRequest materialize_asset = 7;
  }
  reserved 8 to 15;
  reserved "actor", "actor_principal", "principal", "project_id", "admin", "credential";
}
```

The response schema is equally closed. Enum zero values are always unspecified and
invalid in a successful result. Domain token strings retain the TASK-006 1..64-byte
ASCII grammar; logical/media values retain their existing domain bounds.

```proto
enum CoreLiveness {
  CORE_LIVENESS_UNSPECIFIED = 0;
  CORE_LIVENESS_LIVE = 1;
  CORE_LIVENESS_STOPPING = 2;
  CORE_LIVENESS_FAILED = 3;
}
enum CoreReadiness {
  CORE_READINESS_UNSPECIFIED = 0;
  CORE_READINESS_NOT_READY = 1;
  CORE_READINESS_READY = 2;
}
enum CoreAvailability {
  CORE_AVAILABILITY_UNSPECIFIED = 0;
  CORE_AVAILABILITY_FULL = 1;
  CORE_AVAILABILITY_READ_ONLY_CUSTODY = 2;
  CORE_AVAILABILITY_DEGRADED_DEPENDENCY = 3;
  CORE_AVAILABILITY_DEGRADED_CUSTODY = 4;
}
enum LocalSecurityBaseline {
  LOCAL_SECURITY_BASELINE_UNSPECIFIED = 0;
  LOCAL_SECURITY_BASELINE_VERIFIED = 1;
  LOCAL_SECURITY_BASELINE_UNSAFE = 2;
  LOCAL_SECURITY_BASELINE_UNAVAILABLE = 3;
}
enum CustodyObservation {
  CUSTODY_OBSERVATION_UNSPECIFIED = 0;
  CUSTODY_OBSERVATION_UNASSESSED = 1;
  CUSTODY_OBSERVATION_NORMAL_VERIFIED = 2;
  CUSTODY_OBSERVATION_DEEP_VERIFIED = 3;
}
enum ReadinessBlockReason {
  READINESS_BLOCK_REASON_UNSPECIFIED = 0;
  READINESS_BLOCK_REASON_NONE = 1;
  READINESS_BLOCK_REASON_LOCAL_RECOVERY_PENDING = 2;
  READINESS_BLOCK_REASON_LOCAL_RECOVERY_OPERATOR_ACTION = 3;
}

message GetLibraryStatusResult {
  CoreLiveness liveness = 1;
  CoreReadiness readiness = 2;
  CoreAvailability availability = 3;
  LocalSecurityBaseline local_security_baseline = 4;
  bool can_read_metadata = 5;
  bool can_verify = 6;
  bool can_ingest = 7;
  bool can_materialize = 8;
  uint32 staging_orphan_count = 9;
  uint64 staging_orphan_bytes = 10;
  bool local_backend_matches = 11;
  bool observability_degraded = 12;
  bool recovery_observation_available = 13;
  uint64 recovery_required_command_count = 14;
  CustodyObservation custody_observation = 15;
  ReadinessBlockReason readiness_block_reason = 16;
  reserved 17 to 31;
}

message VerifyLibraryResult {
  string verification_id = 1;
  VerificationMode mode = 2;
  uint64 snapshot_commit_sequence = 3;
  uint64 discovered_issue_count = 4;
  uint32 stored_issue_count = 5;
  uint64 dropped_issue_count = 6;
  bool has_fatal_local_issue = 7;
  bool has_custody_degradation = 8;
  bool canonical_extra_classification_deferred = 9;
  optional IntegrityIssue first_fatal_issue = 10;
  reserved 11 to 31;
}

enum IntegrityIssueKind {
  INTEGRITY_ISSUE_KIND_UNSPECIFIED = 0;
  INTEGRITY_ISSUE_KIND_DATABASE_INTEGRITY_FAILURE = 1;
  INTEGRITY_ISSUE_KIND_SCHEMA_OR_MIGRATION_MISMATCH = 2;
  INTEGRITY_ISSUE_KIND_LIBRARY_AUTHORITY_MISMATCH = 3;
  INTEGRITY_ISSUE_KIND_COMMAND_RECOVERY_REQUIRED = 4;
  INTEGRITY_ISSUE_KIND_EVENT_OR_GRAPH_INCONSISTENT = 5;
  INTEGRITY_ISSUE_KIND_LOCAL_BACKEND_MISMATCH = 6;
  INTEGRITY_ISSUE_KIND_MANAGED_BLOB_MISSING = 7;
  INTEGRITY_ISSUE_KIND_MANAGED_BLOB_UNSAFE = 8;
  INTEGRITY_ISSUE_KIND_MANAGED_BLOB_LENGTH_MISMATCH = 9;
  INTEGRITY_ISSUE_KIND_MANAGED_BLOB_DIGEST_MISMATCH = 10;
  INTEGRITY_ISSUE_KIND_UNREGISTERED_CANONICAL_BLOB = 11;
  INTEGRITY_ISSUE_KIND_STAGING_ORPHAN = 12;
  INTEGRITY_ISSUE_KIND_UNSAFE_CAS_NAMESPACE_ENTRY = 13;
  INTEGRITY_ISSUE_KIND_MATERIALIZATION_RECOVERY_REQUIRED = 14;
}
enum IntegritySeverity {
  INTEGRITY_SEVERITY_UNSPECIFIED = 0;
  INTEGRITY_SEVERITY_FATAL_LOCAL = 1;
  INTEGRITY_SEVERITY_READ_ONLY_CUSTODY = 2;
  INTEGRITY_SEVERITY_DEGRADED_CUSTODY = 3;
  INTEGRITY_SEVERITY_OPERATOR_ACTION = 4;
}
enum IntegrityObjectKind {
  INTEGRITY_OBJECT_KIND_UNSPECIFIED = 0;
  INTEGRITY_OBJECT_KIND_LIBRARY = 1;
  INTEGRITY_OBJECT_KIND_COMMAND = 2;
  INTEGRITY_OBJECT_KIND_ASSET = 3;
  INTEGRITY_OBJECT_KIND_ASSET_REVISION = 4;
  INTEGRITY_OBJECT_KIND_BLOB = 5;
  INTEGRITY_OBJECT_KIND_LOCATION = 6;
  INTEGRITY_OBJECT_KIND_STAGING = 7;
  INTEGRITY_OBJECT_KIND_MATERIALIZATION = 8;
}
enum IntegrityRemediation {
  INTEGRITY_REMEDIATION_UNSPECIFIED = 0;
  INTEGRITY_REMEDIATION_NONE = 1;
  INTEGRITY_REMEDIATION_RETRY_EXACT_COMMAND = 2;
  INTEGRITY_REMEDIATION_RERUN_WHEN_IDLE = 3;
  INTEGRITY_REMEDIATION_OPERATOR_CONFIGURATION = 4;
  INTEGRITY_REMEDIATION_FUTURE_ADMIN_ACTION = 5;
  INTEGRITY_REMEDIATION_OPERATOR_OR_RUNTIME_ACTION = 6;
}

message IntegrityIssue {
  uint32 ordinal = 1;
  IntegrityIssueKind kind = 2;
  IntegritySeverity severity = 3;
  IntegrityObjectKind object_kind = 4;
  optional bytes object_id = 5; // absent, 16-byte UUID, or 32-byte digest by kind
  IntegrityRemediation remediation = 6;
  reserved 7 to 15;
}

message ListIntegrityIssuesResult {
  string verification_id = 1;
  repeated IntegrityIssue issues = 2; // at most requested page_size <= 64
  optional bytes next_cursor = 3;      // exactly 96 bytes
  uint64 discovered_issue_count = 4;
  uint32 stored_issue_count = 5;
  uint64 dropped_issue_count = 6;
  reserved 7 to 15;
}

enum AssetLifecycleValue {
  ASSET_LIFECYCLE_VALUE_UNSPECIFIED = 0;
  ASSET_LIFECYCLE_VALUE_ACTIVE = 1;
  ASSET_LIFECYCLE_VALUE_RETIRED = 2;
}
enum RevisionCustodyValue {
  REVISION_CUSTODY_VALUE_UNSPECIFIED = 0;
  REVISION_CUSTODY_VALUE_MANAGED = 1;
  REVISION_CUSTODY_VALUE_UNMANAGED = 2;
}
enum LocationLifecycleValue {
  LOCATION_LIFECYCLE_VALUE_UNSPECIFIED = 0;
  LOCATION_LIFECYCLE_VALUE_AVAILABLE = 1;
  LOCATION_LIFECYCLE_VALUE_CORRUPT = 2;
  LOCATION_LIFECYCLE_VALUE_MISSING = 3;
  LOCATION_LIFECYCLE_VALUE_REMOVED = 4;
}
enum LocationCustodyValue {
  LOCATION_CUSTODY_VALUE_UNSPECIFIED = 0;
  LOCATION_CUSTODY_VALUE_MANAGED = 1;
  LOCATION_CUSTODY_VALUE_UNMANAGED = 2;
}
enum LocationDurabilityValue {
  LOCATION_DURABILITY_VALUE_UNSPECIFIED = 0;
  LOCATION_DURABILITY_VALUE_DURABLE = 1;
  LOCATION_DURABILITY_VALUE_UNKNOWN = 2;
}

message AssetSummary {
  string asset_id = 1;
  string kind = 2;
  AssetLifecycleValue lifecycle = 3;
  uint64 revision = 4;
  int64 created_at_seconds = 5;
  uint32 created_at_nanos = 6;
  uint64 creation_commit_sequence = 7;
  reserved 8 to 15;
}

message ListAssetsResult {
  uint64 snapshot_commit_sequence = 1;
  repeated AssetSummary assets = 2; // at most requested page_size <= 64
  optional bytes next_cursor = 3;   // exactly 80 bytes
  reserved 4 to 15;
}

message AssetMemberView {
  string representation_id = 1;
  string representation_purpose = 2;
  string resource_id = 3;
  string resource_kind = 4;
  uint32 member_ordinal = 5;
  string logical_name = 6;
  bytes blob_sha256 = 7;
  uint64 byte_length = 8;
  optional string media_type = 9;
  optional string location_id = 10;
  LocationLifecycleValue location_lifecycle = 11; // unspecified exactly when absent
  LocationCustodyValue location_custody = 12;     // unspecified exactly when absent
  LocationDurabilityValue location_durability = 13; // unspecified exactly when absent
  reserved 14 to 15;
}

message InspectAssetResult {
  AssetSummary asset = 1;
  string asset_revision_id = 2;
  uint32 revision_sequence = 3;
  string content_kind = 4;
  RevisionCustodyValue custody = 5;
  repeated string parent_revision_ids = 6; // at most 64
  repeated AssetMemberView members = 7;    // member-Location projections, at most page_size <= 64
  optional bytes next_cursor = 8;          // exactly 208 bytes
  reserved 9 to 15;
}

message MaterializeAssetResult {
  string command_id = 1;
  string asset_revision_id = 2;
  string representation_id = 3;
  string resource_id = 4;
  uint32 member_ordinal = 5;
  bytes blob_sha256 = 6;
  uint64 byte_length = 7;
  bool replayed = 8;
  bool cleanup_pending = 9;
  reserved 10 to 15;
}

message CoreResponse {
  oneof response {
    IngestAssetCopyResult ingest_asset_copy = 1;
    GetLibraryStatusResult get_library_status = 2;
    VerifyLibraryResult verify_library = 3;
    ListIntegrityIssuesResult list_integrity_issues = 4;
    InspectAssetResult inspect_asset = 5;
    ListAssetsResult list_assets = 6;
    MaterializeAssetResult materialize_asset = 7;
    ErrorEnvelope error = 15;
  }
  reserved 8 to 14;
}
```

The accepted `.proto` must retain these explicit values and reserve any removed
names; `CoreResponse` maps tags 2..7 to these six results and retains error tag 15.
The client validates every successful response:
canonical IDs/digests/timestamps/revisions, enum values, repeated bounds, cursor
length/checksum and the request/result identity relationship. Any malformed or
wrong-operation success after request transmission is uncertain
`IPC_TRANSPORT_ERROR`, never a partially trusted display.

For a `MANAGED` revision, every Member must resolve at least one policy-required
durable custody Location at its captured Blob revision and therefore returns
one projection per qualifying Location; absence is corruption. Other valid Location
custody/durability/lifecycle values remain visible as separate safe projections and
are not silently collapsed into the selected materialization Location. An
`UNMANAGED` revision with no Location returns exactly one absent-Location projection,
and all three Location enums must then be unspecified. Inspecting that metadata does
not promote it, and Materialize rejects it because it cannot satisfy the
managed-current-backend selection.

`CORRUPT`, `MISSING` and `REMOVED` remain in `LocationLifecycleValue` because they
are valid canonical migration-0001 values and future readers must not reinterpret
them as unknown. TASK-008 maps them read-only and tests each value; it has no code
path authorized to write any Location lifecycle.

Every result uses typed scalar fields and bounded repeated fields. No result echoes
destination bytes or contains a backend ID/locator. Cursors are bytes on wire and
lowercase hexadecimal in CLI output/input. A cursor larger than its exact format,
an invalid ID, zero page size, page size above 64, unknown enum, unexpected cursor
on a first-page-only operation, or malformed canonical byte path fails before any
store/storage action.

### 5.1 Thin CLI grammar and output

The exact new command grammar is:

```text
mengxia library status [existing endpoint/config/handshake flags]
mengxia library verify --mode normal|deep [--operation-timeout-ms N] [common flags]
mengxia library issues --verification-id UUID [--page-size N] [--cursor HEX]
                       [--operation-timeout-ms N] [common flags]
mengxia asset list [--page-size N] [--cursor HEX]
                   [--operation-timeout-ms N] [common flags]
mengxia asset inspect --asset-id UUID [--asset-revision-id UUID]
                      [--page-size N] [--cursor HEX]
                      [--operation-timeout-ms N] [common flags]
mengxia asset materialize --command-id UUID --asset-id UUID
                          --asset-revision-id UUID --representation-id UUID
                          --resource-id UUID
                          --member-ordinal N --destination ABSOLUTE_PATH
                          [--operation-timeout-ms N] [common flags]
```

`common flags` are only the already accepted client Library-config, endpoint,
frame/depth/handshake and timeout selectors. Duplicate/unknown/missing flags,
unexpected positionals, invalid hex/cursor/ID/number/path or mode fail locally with
exit 2 before connection. Application/transport errors exit 1; success exits 0.
There is no `--all`, automatic page traversal, overwrite, output-directory,
recursive, JSON, Admin or raw-locator escape hatch.

Each invocation emits fixed ASCII record kinds (`MENGXIA_LIBRARY_STATUS`,
`MENGXIA_VERIFY_OK`, `MENGXIA_INTEGRITY_ISSUE`, `MENGXIA_ASSET`,
`MENGXIA_ASSET_MEMBER`, `MENGXIA_MATERIALIZE_OK`) with fixed ordered fields and at
most one page. `MENGXIA_ASSET_MEMBER` is one member-Location projection; repeated
Member IDs represent distinct Location IDs, while the unmanaged no-Location sentinel
prints `location_id=-` and all Location enums as `unspecified`. Cursors, Blob digests
and authorized logical-name UTF-8 bytes are
lowercase hex (`logical_name_hex=`); the already restricted media type/token values
are emitted in their canonical safe ASCII grammar. Output never echoes
source/destination, logical name in a log context, backend/locator/CAS root,
SQL/error details or config values. Inspect values are stdout product data, not
stderr structured logging. The raw destination flag uses `OsStrExt::as_bytes`
without UTF-8 replacement and is never copied into a String.

### 5.2 Operation API-010 matrix

| Operation | Authority | Side effect | Idempotency/retry | Deadline/cancel | Pagination |
|---|---|---|---|---|---|
| `GetLibraryStatus` | authenticated Library owner Client; sole operation served while startup is NOT_READY | none | naturally repeatable | transport deadline only | none |
| `VerifyLibrary` | authenticated Library owner Client | bounded in-memory report only | fresh request starts a fresh report; never auto-retried after uncertain transport | operation deadline; cooperative checkpoints; no partial report | report issues listed separately |
| `ListIntegrityIssues` | authenticated Library owner Client | none | repeatable while report lives | transport/read deadline | report-bound cursor |
| `InspectAsset` | authenticated Library owner Client | none | repeatable; cursor binds selected revision/Asset revision and current-Member Blob revision | transport/read deadline | graph-member/Location cursor |
| `ListAssets` | authenticated Library owner Client | none | repeatable within snapshot | transport/read deadline | event-sequence cursor |
| `MaterializeAsset` | authenticated Library owner Client plus validated destination authority | one no-clobber file | durable command binding; same request replays; mismatch conflicts | cancellable only before publish; post-publish completes/requires recovery | none |

Request/command principal always comes from the existing authenticated session and
durable Library owner metadata. No operation accepts an actor, role, tenant, Project
or Admin claim. V1 remains a single-Library/single-owner trust domain.

## 6. Exact pagination and cursor contracts

All integers in cursors are unsigned big-endian. Reserved bytes must be zero. The
last 32 bytes are SHA-256 over every preceding byte. The checksum is corruption and
cross-operation protection, not authorization; every request is independently
authenticated and every embedded ID/snapshot is revalidated. Cursor structures
have no public field decoder in CLI or app API. On an Inspect continuation the
request Asset ID must equal the embedded Asset ID; an explicitly repeated
AssetRevision ID must equal the embedded selected revision, while omission means
“use the cursor selection” rather than reselect latest. On an issue continuation the
request verification ID must equal the embedded ID. Those caller/cursor mismatches
are `VALIDATION_ERROR`; a valid cursor whose captured Asset revision changed is the
separate `CONFLICT` case.

### 6.1 ListAssets cursor, 80 bytes

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | magic hex `4d584c4355523100` (`MXLCUR1\0`) |
| 8 | 2 | format version = 1 |
| 10 | 2 | total length = 80 |
| 12 | 4 | operation discriminator = 1 |
| 16 | 16 | durable Library ID |
| 32 | 8 | captured `last_sequence` |
| 40 | 8 | last examined event commit sequence; non-zero in every emitted cursor; absent-cursor initial state is zero |
| 48 | 32 | SHA-256 of bytes 0..48 |

First request has absent/empty cursor. In the same read transaction the store reads
the allocator and the unique-index maximum event sequence, requires exact equality,
and captures that value. An empty event table is valid only with allocator/snapshot
zero and returns an empty terminal page. A continuation cursor is valid only when
`0 < last examined < captured snapshot <= 9223372036854775807`; it additionally
requires the current allocator not to have moved below the snapshot and requires an
event at both the last-examined and exact snapshot sequences. These consistency
checks use singleton or unique-index endpoint/point lookups and do not consume the
256 stepped-event budget.
Allocator/event mismatch is `STORAGE_CORRUPTION`, while invalid cursor numeric
ordering/range is `VALIDATION_ERROR`. The store then steps events in ascending unique
`commit_sequence` and applies the exact format-1
`asset.registered.v1`/schema-1/ASSET predicate and CommandRecord/Asset validation
from §2.1. It stops before stepping a 257th event and immediately after the event
that yields the `page_size`th Asset, whichever comes first. `next_cursor` records
that actual last stepped event, so a dense 256-event window cannot drop qualifying
Assets after a full response page. It is absent exactly when the last stepped
sequence reaches the captured snapshot, even when the final page contains no Asset.
If no event is returned while the last examined sequence is below the snapshot, the
store returns `STORAGE_CORRUPTION`; it never emits an unchanged cursor or advances to
an event it did not step. Each stepped sequence must be exactly one greater than the
previous stepped/cursor sequence; an interior gap is the same corruption class.
There is no wall-clock expiry:
format version 1 is explicitly bound to migration/schema generation 0001 in the
decoder. A later accepted schema generation must use a new cursor format version and
reject version 1 rather than reinterpret it. The cursor remains valid only for the
same Library ID and that format/schema binding; malformed/cross-Library cursors
return `VALIDATION_ERROR`. Concurrent newly created Assets have sequence above the
snapshot and appear only in a new listing. Existing item fields are read current
and include their revision. A missing row or duplicate/malformed creation event is
`STORAGE_CORRUPTION`, never a skipped item.

### 6.2 InspectAsset cursor, 208 bytes

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | magic hex `4d58494355523100` (`MXICUR1\0`) |
| 8 | 2 | format version = 1 |
| 10 | 2 | total length = 208 |
| 12 | 4 | operation discriminator = 2 |
| 16 | 16 | durable Library ID |
| 32 | 16 | Asset ID |
| 48 | 16 | selected AssetRevision ID |
| 64 | 8 | captured Asset revision number |
| 72 | 16 | current Representation ID; zero before first hierarchy seek |
| 88 | 16 | current Resource ID; zero before first hierarchy seek |
| 104 | 4 | current member ordinal; zero before first hierarchy seek |
| 108 | 4 | phase: 0 before first Member; 1 enumerating/no Location seen; 2 enumerating/Location seen but no required managed-durable custody; 3 enumerating/required custody seen |
| 112 | 8 | captured current-Member Blob revision; zero only in phase 0 |
| 120 | 16 | last returned current-Blob Location ID; zero at start of current Member |
| 136 | 40 | zero reserved bytes |
| 176 | 32 | SHA-256 of bytes 0..176 |

The first page selects an explicitly requested revision or the highest immutable
revision sequence and validates the complete parent set (maximum 64). Members are
keyset-ordered by `(representation_id, resource_id, ordinal)` through separate
bounded existing-index seeks, never a full joined sort. For the current Member, the
adapter enumerates only its Blob's Locations in existing-index
`(backend_id, location_id)` order and emits one projection per Location. The first
seek uses `blob_digest` plus a `remaining page slots + 1` limit. On continuation, the
adapter resolves the last returned Location ID through the primary key, validates
that it still names the current Blob, retains its backend ID only as local query
state, and performs the §2.2 row-value seek with the same bounded limit. The cursor
therefore remains stateless and backend-free while SQL work remains proportional to
the current Blob's Location results rather than the whole Library.

The bounded page transaction captures the Member Blob revision before the first
Location seek and repeats it in every continuation cursor. Phase 0 is valid only
with zero hierarchy/Blob-revision/location fields. Phases 1..3 require a valid
current Member and non-zero valid Blob revision; the zero Location ID means its
enumeration has not started. Phase is monotonic for that Member: 1 -> 2 when any
Location is emitted and 1/2 -> 3 when a `MANAGED`/`DURABLE` Location is emitted.
This preserves custody evidence across multi-page enumeration without retaining
server session state. When the current Blob's indexed Location range is exhausted,
phase 3 proves required managed custody; phase 2 proves a non-empty but
non-qualifying Location set; phase 1 proves no Location. The adapter emits the single
no-Location projection only for a valid UNMANAGED phase-1 Member, rejects a MANAGED
Member that did not reach phase 3 as corruption, otherwise advances to the next
Member and resets phase/location key. Unlike the separately bounded ListAssets
residual event scan, a valid non-final Inspect page cannot be empty: every indexed
Location row qualifies for the current Blob, and a zero-Location UNMANAGED Member
emits its sentinel immediately.

Later pages require the same Asset row revision, selected revision and current-Member
Blob revision. A concurrent Asset lifecycle/revision or current-Member Location
addition returns `CONFLICT`; current Location lifecycle/revision is returned for the
stable membership. When enumeration advances to another Member, its current Blob
revision becomes the new cursor value. Revision graph rows themselves are immutable.
A missing node, empty required collection, duplicate identity, invalid Blob revision,
Member without Blob, graph cap violation or managed Member without required durable
custody is `STORAGE_CORRUPTION`.

### 6.3 Integrity issue cursor, 96 bytes

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | magic hex `4d58564355523100` (`MXVCUR1\0`) |
| 8 | 2 | format version = 1 |
| 10 | 2 | total length = 96 |
| 12 | 4 | operation discriminator = 3 |
| 16 | 16 | durable Library ID |
| 32 | 16 | verification ID |
| 48 | 4 | next issue ordinal |
| 52 | 12 | zero reserved bytes |
| 64 | 32 | SHA-256 of bytes 0..64 |

Issue order is discovery ordinal within the immutable completed report. A report
uses one-based ordinals and is never modified after publication. A next cursor
contains the next one-based ordinal. Cursor expiry is restart/FIFO eviction; a
validly formed cursor for a missing report returns `NOT_FOUND`.

Page size defaults to 32 when the CLI omits it and is accepted from 1 through 64.
The wire request always contains the explicit resolved value. Response construction
must prove its encoded length fits the configured frame before allocation/write;
an internal size miscalculation fails the runtime rather than truncating a row.

## 7. Verification and recovery contract

### 7.1 Verification snapshot and selection

At admission the verifier captures the Library ID, mode and current
`event_commit_sequence.last_sequence` using one bounded read job. It scans metadata
in keyset pages of at most 256 rows and releases the SQLite read connection between
pages; no long-lived read transaction may cause unbounded WAL retention. Membership
for managed Blobs/Locations is limited to rows introduced by a completed command at
or below the captured snapshot. For an ingest result this is the command's unique
`asset.registered.v1` DomainEvent joined through `commands.result_location_id` and
the matching ProvenanceEvent; for a later location result it is the command's
unique `blob.location.recorded.v1` DomainEvent joined through its result Location.
The minimum qualifying sequence is the Blob membership sequence when several
Assets share it. Current row values are validated on each page and their revisions
are reported. A concurrent
post-snapshot mutation is not silently attributed to the snapshot; a revision/event
mismatch produces a typed issue or `CONFLICT` according to whether the canonical
row is invalid or merely changed.

The physical namespace is immutable for promoted TASK-005 Blobs. Normal mode opens
the exact locator descriptor-first and checks backend identity, shard/name case,
file type, owner, mode, ACL, device, link count and byte length. Deep mode additionally
streams exactly `byte_length`, probes EOF, checks pre/post inode metadata and compares
SHA-256. It never follows a path stored outside the validated local-backend grammar.

Physical “unregistered canonical Blob” classification uses a final concurrency
guard. The verifier captures the event sequence and current active-ingest count
before namespace enumeration, then reads both again after enumeration. If the
sequence changed or an ingest was active at either boundary, physical extras are
not reported as orphans and the report sets
`canonical_extra_classification_deferred=true`; registered-Blob checks remain
valid. A client may rerun after mutations settle. If both guards are stable, every
extra is rechecked against current metadata immediately before issue publication.
This prevents the valid TASK-007 promote-before-registration interval from being
misreported. The observation seam exposes only an atomic active count, never a
source path, request digest or Blob identity, and cannot pause or authorize ingest.

### 7.2 Typed issue registry

The closed V1 issue kinds are:

```text
DATABASE_INTEGRITY_FAILURE          FATAL_LOCAL
SCHEMA_OR_MIGRATION_MISMATCH        FATAL_LOCAL
LIBRARY_AUTHORITY_MISMATCH          FATAL_LOCAL
COMMAND_RECOVERY_REQUIRED           OPERATOR_ACTION
EVENT_OR_GRAPH_INCONSISTENT         FATAL_LOCAL
LOCAL_BACKEND_MISMATCH              READ_ONLY_CUSTODY
MANAGED_BLOB_MISSING                DEGRADED_CUSTODY
MANAGED_BLOB_UNSAFE                 DEGRADED_CUSTODY
MANAGED_BLOB_LENGTH_MISMATCH        DEGRADED_CUSTODY
MANAGED_BLOB_DIGEST_MISMATCH        DEGRADED_CUSTODY
UNREGISTERED_CANONICAL_BLOB         OPERATOR_ACTION
STAGING_ORPHAN                      OPERATOR_ACTION
UNSAFE_CAS_NAMESPACE_ENTRY          READ_ONLY_CUSTODY
MATERIALIZATION_RECOVERY_REQUIRED   OPERATOR_ACTION
```

Remediation is selected from the proven operation-specific state, not from issue
kind alone. `COMMAND_RECOVERY_REQUIRED` originating from ingest carries
`OPERATOR_OR_RUNTIME_ACTION`. `MATERIALIZATION_RECOVERY_REQUIRED` carries
`RETRY_EXACT_COMMAND` only when the materialize classifier has proved an exact
recoverable prefix; invalid intent, unknown origin or unsafe authority instead uses
operator configuration/action and cannot invite a futile retry.

The enclosing verification result/report carries `verification_id`; each
`IntegrityIssue` contains only ordinal, kind, severity, a closed object kind and an
optional typed object identifier (UUID or SHA-256 digest), plus a static safe
remediation enum. The identifier is not duplicated inside the issue proto. Neither
shape contains a raw path/name, backend locator, UID, mode, ACL principal, errno,
SQL, source bytes, destination bytes, content metadata or dynamic message. Error
envelopes return at most the verification/issue identifier, never the hidden
evidence.

### 7.3 Corruption and availability matrix

| Observation | Operation result | Runtime effect | Automatic mutation |
|---|---|---|---|
| SQLite cannot open/recover, schema/checksum/quick/FK invalid | `STORAGE_CORRUPTION` or safe configuration/I/O code | endpoint not published at startup; current runtime fails if detected later | none |
| Library owner/lock/path authority changes | safe configuration/corruption code | fatal shutdown; no reads or mutations | none |
| local backend identity changed | report `LOCAL_BACKEND_MISMATCH` | metadata status/inspect/list/verify only; ingest/materialize disabled | none |
| prior staging orphan | report and retain TASK-005 byte accounting | metadata reads/verify remain available; ingest remains subject to its existing capacity/admission result | none |
| registered Blob missing/unsafe/wrong length/hash | typed issue; affected materialize returns `STORAGE_CORRUPTION` | unrelated metadata reads remain; custody-degraded flag set | none |
| unregistered canonical Blob | typed issue | no ownership inferred; no Asset exposed | none |
| prior ingest `CLAIMED`/`RECOVERY_REQUIRED` command | typed issue with `OPERATOR_OR_RUNTIME_ACTION` | exact retry cannot reacquire through the accepted ingest port | existing ingest/operator-specific handling only |
| prior materialize `CLAIMED`/recoverable `RECOVERY_REQUIRED` command | typed issue; `RETRY_EXACT_COMMAND` only after the §2.10 physical classifier | exact same authenticated request may attempt CAS reacquire | materialize-specific classifier plus CAS only |
| Provider unavailable future seam | typed Provider-degraded reason | local reads and allowed local mutations remain ready | none |

`FATAL_LOCAL` can never be converted to `DEGRADED` merely to keep the endpoint
available. Conversely, a missing Provider cannot mark SQLite/CAS corrupt or block
unrelated local capability. This distinction is enforced in the health state
constructor rather than inferred from log text.

If an explicit verification safely classifies a fatal issue, it seals the bounded
report, places the first safe fatal issue directly in `VerifyLibraryResult`,
atomically closes all new read/mutation admission, writes that one terminal result
when transport is still healthy, and then enters joined fatal shutdown. The report
is not promised listable after that response because no later request is served. If
SQLite/platform failure prevents safe issue construction, the request receives only
the static `STORAGE_CORRUPTION`, configuration or I/O envelope and the same shutdown
occurs; fabricated partial evidence is forbidden.

### 7.4 Finite execution

At most one verification executes. A second request returns pre-effect
`BACKPRESSURE`. Store pages contain at most 256 rows, issue storage at most 4096,
completed report storage at most four and filesystem/hash buffers reuse the accepted
TASK-005 stream buffer cap. Every row, directory entry and Blob chunk is a
cooperative deadline/cancellation checkpoint. No retry is performed automatically.
Worker panic/join/channel loss closes verification and current runtime with
`INTERNAL_ERROR`; no partial report is published. Summary counters use checked u64
arithmetic; overflow is an internal invariant failure, never wrapping or truncation.

### 7.5 Exact SQLite deadline and cancellation

Checking a token between returned rows is not sufficient because a SQLite statement
may still be executing. Every TASK-008 read/verification/startup-classification job
therefore owns a cancellation state and obtains rusqlite's already available
`Connection::get_interrupt_handle()` before the blocking statement begins. No
`hooks` feature, manifest edit, new dependency or raw connection is exposed. One
joined daemon watcher observes the monotonic operation deadline, authenticated
connection disconnect and daemon shutdown; the first terminal signal atomically
sets exactly one cancellation cause and calls `interrupt()` on that job's handle.
Cancellation before dispatch consumes no read slot or worker job.

After the single operation frame is decoded, the transport retains its read half in
that watcher. No second client frame is legal on the one-operation connection, so
EOF/reset is disconnect cancellation and any additional byte is a protocol failure
with the same pre-response cancellation path. The write half remains owned by the
response task. Dropping only the client write half is therefore documented as
cancelling the operation, not as a supported half-close request pattern. Startup S7
has no connection cause and observes only its fixed deadline and daemon shutdown.

`SQLITE_INTERRUPT` caused by that owned state maps to `DEADLINE_EXCEEDED` when the
monotonic deadline won, and otherwise to `OPERATION_CANCELLED`. It is never mapped to
`STORAGE_CORRUPTION`, `STORAGE_IO_ERROR` or a partial success. An unrelated SQLite
interrupt without the owned cancellation state is `INTERNAL_ERROR` and closes the
runtime. If the statement completes before a cancellation cause is committed, its
fully validated result may win; otherwise cancellation wins and no report/page is
published. All statements are finalized and the blocking job is joined before its
terminal result is returned.

After any interrupt, the read connection is removed from service, closed and
replaced through the existing bounded OpenedLibrary read-worker construction and
schema/runtime validation before another request may use that slot. A replacement
failure closes read admission and follows the typed fatal/degraded matrix; a possibly
interrupted connection is never blindly reused. Explicit NORMAL/DEEP verification
uses this mechanism for its initial `PRAGMA quick_check`, metadata statements and
page scans. S7 uses it for its bounded scans/transactions. Materialize write
transactions remain short and use the existing busy budget; cancellation is checked
before each transaction, while the post-publish M9+ rule still forbids cancellation
from overriding durable completion/recovery.

## 8. Asset read model

The store exposes a provider-neutral `AssetQueryPort`; no row or SQL handle leaves
the adapter. Expected public shapes are conceptually:

```rust
pub trait AssetQueryPort: Send + Sync {
    fn list_assets(&self, request: ListAssetsQuery) -> AssetQueryFuture<'_, AssetPage>;
    fn inspect_asset(&self, request: InspectAssetQuery) -> AssetQueryFuture<'_, AssetMemberPage>;
    fn resolve_materialization(&self, request: MaterializationSelection)
        -> AssetQueryFuture<'_, ResolvedManagedMember>;
}
```

`ResolvedManagedMember` is an internal ownership value. It carries typed graph IDs,
digest, length and one store-validated managed Location descriptor. It has no
Debug/Display/serialization and cannot be constructed by transport/CLI code.
Storage-local consumes it and independently revalidates the exact backend/locator;
the app cannot convert it to a filesystem path.

`InspectAsset` returns all snapshot-member Location projections through §6.2; it does
not reuse the materializer's single-current-backend selector. `InspectAsset` and
`ListAssets` are read-only. They never claim a command, update a timestamp, repair a
row or append an event. Query backpressure returns
`BACKPRESSURE`; `BUSY` uses `STORAGE_BUSY`; malformed canonical rows fail
`STORAGE_CORRUPTION`. `NOT_FOUND` is returned only after syntactically valid,
authorized IDs select no canonical object.

Resolution additionally requires the Resource to belong to a Representation under
the selected AssetRevision, the ordinal to select exactly one Member, the Blob to
be `AVAILABLE`, and exactly one usable `MANAGED`/`DURABLE`/`AVAILABLE` Location for
the currently opened local backend. No matching current backend is
`STORAGE_CONFIGURATION_ERROR`; a matching metadata row whose physical file is
missing/unsafe/mismatched is `STORAGE_CORRUPTION`. Multiple equivalent current
Locations are rejected as corruption rather than selected nondeterministically.

## 9. MaterializeAsset semantic binding

Accepted request fields and digest participation are:

| Field | Validation | Digest value |
|---|---|---|
| `command_id` | canonical typed UUIDv7 | excluded; it keys the binding |
| Asset ID | canonical typed UUIDv7 | 16 raw bytes |
| AssetRevision ID | canonical typed UUIDv7; belongs to Asset | 16 raw bytes |
| Representation ID | canonical typed UUIDv7; belongs to selected revision | 16 raw bytes |
| Resource ID | canonical typed UUIDv7; belongs to selected Representation | 16 raw bytes |
| member ordinal | 0..4095 and exists exactly once | u32 big-endian |
| destination | normalized absolute raw Unix bytes, 1..1023, NUL-free | destination selector digest |
| operation timeout | 100..accepted server ceiling | excluded |
| principal | existing server/durable owner UID only | bound separately by CommandRecord |

The destination selector digest is:

```text
SHA-256(
  ASCII "MENGXIA_DESTINATION_SELECTOR_V1" || 0x00 ||
  u16_be(path_byte_length) || path_bytes
)
```

The canonical request digest is:

```text
SHA-256(
  ASCII "MENGXIA_ASSET_MATERIALIZE_MEMBER_REQUEST_V1" || 0x00 ||
  TLV(0x01, asset_id_16) ||
  TLV(0x02, asset_revision_id_16) ||
  TLV(0x03, representation_id_16) ||
  TLV(0x04, resource_id_16) ||
  TLV(0x05, member_ordinal_u32_be) ||
  TLV(0x06, destination_selector_digest_32) ||
  TLV(0x07, ASCII "NO_REPLACE_FILE_V1")
)
```

TLV is the accepted TASK-007 `u8 tag || u32_be(length) || value` encoding with
strictly increasing, exactly-once tags. Operation ID remains the canonical
`asset.materialize.v1`. Golden vectors cover non-Unicode paths, every field,
path spelling differences, boundary ordinals and one-bit changes.

The response is the canonical exact-request replay-result view and contains Command ID,
AssetRevision ID, Representation ID, Resource ID, member ordinal, Blob digest, byte
length, `replayed` and `cleanup_pending`. The durable result fields are stable;
the last two booleans describe the current delivery/recovery observation and may
change on a later exact replay. It never returns the destination, backend, locator
or CAS identity. Materializing the same bytes to another path requires a fresh
command ID because it is a different effect.

The materialize-specific observation matrix is exact and does not call the existing
external-ingest claim path. After authenticated scalar/path-syntax validation and
request-digest construction, command observation occurs before destination
existence/authority is interpreted. This permits deterministic replay of a success
whose final file normally exists and prevents a changed destination from hiding a
durable terminal result:

| Existing durable state for the same binding | Outcome |
|---|---|
| absent | materialize-specific transaction inserts current-runtime `CLAIMED`, return `NEW` |
| current-runtime `CLAIMED` | `COMMAND_IN_PROGRESS` |
| prior-runtime `CLAIMED` | return `RECOVERY_CANDIDATE`; mutate nothing until physical classification |
| matching `RECOVERY_REQUIRED` with recoverable materialize safe code | return `RECOVERY_CANDIDATE`; mutate nothing until physical classification |
| `RECOVERY_REQUIRED` with an invalid materialize code/state | `STORAGE_CORRUPTION`; never reacquire |
| `COMPLETED/ASSET_REVISION` for `asset.materialize.v1` with selected revision | return exact-request replay result; retry exact sidecar cleanup if present |
| matching `TERMINAL_REJECTED` | replay the stored safe code/action |
| any different principal/operation/digest or impossible result | `CONFLICT` or fatal corruption as appropriate |

`COMPLETED` and `TERMINAL_REJECTED` never enter the new-effect path. A terminal
rejection is returned from the ledger without filesystem access. A completed replay
revalidates the immutable graph/result and returns stored success regardless of
whether the user later retained, removed or changed the final. It may additionally
inspect and remove only an exact valid intent when the destination parent can still
be proven safe. Failure to prove safe cleanup cannot reverse success and reports
`cleanup_pending=true`; proven absence or successful durable cleanup reports false.
It never requires the final target to be absent and never recreates it.

The ports expose separate typed methods for
`observe_materialization`, `claim_new_materialization`,
`reacquire_materialization`, `complete_materialization` and
`finish_materialization`. Observation owns no mutation guard. A
`MaterializationClaimGuard` is created only after a successful new claim or
compare-and-swap reacquire; its Drop/failure path marks only unresolved
current-runtime materialize claims through the dedicated store method and then
triggers the accepted fatal mutation gate. None of these symbols is an alias for
`ExternalIngestClaim`, `ExternalClaimOutcome` or `ExternalClaimGuard`.

For `NEW`, and only after observation has proved the row absent, the app resolves the
exact Member/current backend and requires a safe destination with an absent final.
For `RECOVERY_CANDIDATE`, it first validates the §11 state without deleting or
publishing anything. A matching prior-runtime row with no intent is recoverable only
for the claim-before-intent prefix. A matching valid intent authorizes only its exact
state. The subsequent materialize-specific compare-and-swap reacquire sets the
current runtime, `CLAIMED` and `safe_error_code=NULL` in one transaction. Only one
concurrent caller succeeds; losers observe current-runtime in-progress. An invalid
intent or unprovable physical state leaves the durable row unchanged and fails
closed.

Completing materialization updates only the existing CommandRecord result. It does
not change Asset/Revision/Blob/Location state and therefore emits no DomainEvent or
ProvenanceEvent. The external-file observation is represented by the durable command
outcome plus destination intent protocol; TASK-008 does not invent an audit stream
before the separately authorized SecurityAuditEvent task.

The store adds a materialize-specific replay entry point and mapper; it must not
route through or alter the existing
`asset.revision.create.v1` mapper, which correctly requires that command's
`asset.revision.created.v1` event. The new entry point dispatches on operation ID
before interpreting result kind. Its mapper validates binding,
`result_kind='ASSET_REVISION'`, exact selected revision ID and the still-consistent
Asset/Representation/Resource/Member relationship from the same request.
Representation, Resource, ordinal and destination are recovered only from the
revalidated request whose versioned digest matches the durable row; they are not
guessed from the result ID.
Missing/mismatched canonical rows are `STORAGE_CORRUPTION`.

Materialize samples `claimed_at` immediately before a new claim; recovery samples
`reacquired_at` before its compare-and-swap; and it samples one `completed_at` only
after M10 when ready to commit M11. A failed sample before claim/reacquire causes no
state change. After claim/reacquire, the most recent trusted command timestamp may be
reused as `updated_at` solely to persist a terminal/recovery disposition when a new
clock sample itself fails; the runtime never fabricates a later time. Recovery that
has not completed samples a new completion time after revalidating the durable final.
These times are used only for CommandRecord timestamps, not for file authority or the
request digest. Clock failure before publish follows proven cleanup and terminal
`ID_GENERATION_UNAVAILABLE`; clock failure after publish is recovery-required and
must not relabel the visible effect as absent.

## 10. Destination authority and physical protocol

### 10.1 Destination validation

The raw destination is converted losslessly with `OsStringExt`. It must be an
absolute normalized path of 1..1023 bytes, contain no NUL, repeated slash, `.` or
`..`, and have a final component of 1..255 bytes. `/`, trailing slash and the fixed
reserved materialization sidecar patterns are invalid.

Platform-fs walks every component from `/` with retained no-follow directory
descriptors. Every traversed filesystem must be local APFS with ownership checking
enabled. A mount transition is accepted only when it is detected and the new mount
independently satisfies that policy; network/non-APFS/ownership-disabled mounts are
rejected. Each ancestor
is root/eUID-owned, has no allow/inheritable/unknown ACL and is not group/world
writable. A conforming non-inheritable deny-only ACL is accepted. The final parent
must be eUID-owned mode 0700 with an empty ACL. For a `NEW` command the final target
must be absent. Exact valid-intent recovery accepts only the §11 final state for its
proven prefix. A completed replay does not apply an absence precondition: it never
republishes and may inspect only for exact sidecar cleanup. Symlink, unvalidated mount crossing, case mismatch,
hard-link anomaly, parent replacement or inability to prove policy fails before
publish. This deliberately rejects broad shared/tmp destinations; TASK-008 does not
invent group collaboration policy.

The destination authority also receives opaque exclusion identities for the opened
Library and Blob roots. If any retained destination ancestor/final parent is either
root or lies beneath either managed root, validation fails. Thus materialize cannot
write into the Library namespace, CAS shards/staging, SQLite files or lock/intent
names even when the caller knows their ordinary path spelling. These exclusions are
device/inode/ancestor-authority comparisons, not raw-path string prefixes.

The retained parent descriptor is revalidated after command claim, before staging
creation, before publish, after publish and before cleanup. The created intent,
staging and final file are eUID-owned regular files, mode 0600, empty ACL, same
device and link count one. File creation passes exact mode and then revalidates;
ambient umask or inherited ACL cannot weaken the check.

### 10.2 Fixed names

For canonical lowercase 32-hex command ID `c`:

```text
.mengxia-materialize-c.intent
.mengxia-materialize-c.staging
```

Both names are derived internally; caller input cannot select them. They are
created with no-follow/exclusive semantics. In a NEW/RECOVERY effect path, any
collision without an exact valid intent for the same command/request/parent/final
selection is `STORAGE_CONFIGURATION_ERROR` and no name is removed. A completed
replay cannot have its durable success replaced by that error: an invalid/unprovable
sidecar is retained, produces static operator action and `cleanup_pending=true`.

### 10.3 Materialization intent wire format, exactly 512 bytes

All integers are unsigned big-endian. Every reserved byte must be zero.

| Offset | Width | Field |
|---:|---:|---|
| 0 | 16 | magic hex `4d454e475849415f4d41545f56310000` |
| 16 | 2 | version = 1 |
| 18 | 2 | record length = 512 |
| 20 | 4 | flags = 0 |
| 24 | 16 | command ID |
| 40 | 16 | Asset ID |
| 56 | 16 | AssetRevision ID |
| 72 | 16 | Representation ID |
| 88 | 16 | Resource ID |
| 104 | 4 | member ordinal |
| 108 | 4 | zero reserved |
| 112 | 32 | expected Blob SHA-256 |
| 144 | 8 | expected byte length |
| 152 | 8 | destination parent device |
| 160 | 8 | destination parent inode |
| 168 | 2 | final basename byte length, 1..255 |
| 170 | 255 | final basename bytes then zero padding |
| 425 | 7 | zero reserved |
| 432 | 32 | canonical request digest |
| 464 | 16 | durable Library ID |
| 480 | 32 | SHA-256 of bytes 0..480 |

The accepted implementation must include a checked-in golden 512-byte fixture and
independent checksum/offset tests. Device/inode values must convert exactly to u64;
overflow or negative platform values fail closed. The parser requires exact EOF,
valid typed IDs/digest, valid ordinal/length/name, zero padding/reserved fields,
matching checksum and equality with the current request plus retained parent.

### 10.4 Ordered effect protocol

```text
M0 validate authenticated scalar/path syntax, compute binding and observe the
   CommandRecord; dispatch COMPLETED/TERMINAL_REJECTED/CONFLICT without entering a
   new effect; completed replay may perform only exact valid-intent cleanup
M1 for NEW/RECOVERY_CANDIDATE only, resolve the exact managed member/current local
   backend and validate destination authority; NEW requires final absent while
   recovery requires its exact §11 physical prefix
M2 durably claim NEW or compare-and-swap reacquire the classified recovery row
M3 revalidate parent and exclusively create intent
M4 write exact 512 bytes, verify exact EOF/read-back/checksum and inode/name,
   F_FULLFSYNC(intent), F_FULLFSYNC(parent), reopen and revalidate the same intent
M5 exclusively create and revalidate empty staging; F_FULLFSYNC(staging), then
   F_FULLFSYNC(parent)
M6 open managed CAS file descriptor-first; copy bounded chunks while hashing
M7 verify exact length, EOF, digest and source pre/post identity
M8 F_FULLFSYNC(staging); revalidate staging, source and parent
M9 renameat no-replace staging -> final in same parent
M10 revalidate exact final inode/content and parent, F_FULLFSYNC(parent), then
    revalidate both; final is now externally visible and durable
M11 complete CommandRecord as ASSET_REVISION
M12 attempt to unlink only the exact valid intent; F_FULLFSYNC(parent)
M13 return stored success with `cleanup_pending` reflecting M12
```

There is no cross-device fallback because staging and final share the retained
parent. No overwrite option exists. Before M9, cancellation/deadline may clean the
valid-intent-owned staging/intent durably and store a terminal rejection. From M9
onward, cancellation/deadline is observed but cannot override M10..M12; completion
or recovery is mandatory.

If cleanup before publish is uncertain, the command is `RECOVERY_REQUIRED`, not a
clean retry. If M10 succeeded but M11 cannot be proved, the command remains/reverts
to recovery-required and current mutation admission fails closed. Once M11 commits,
the canonical operation outcome is success. M12 failure cannot reverse the already
durable external effect/result: it returns success with `cleanup_pending=true`,
emits only a static operator-action observation, and retains the valid intent as
recovery evidence. Exact replay may retry only that intent cleanup and returns the
same stored result. A completed replay with no sidecar performs no filesystem write
and never rematerializes.

## 11. Materialize restart and failure matrix

| Crash/failure prefix | Durable/visible state on same-OS SIGKILL restart | Exact same-command action |
|---|---|---|
| before M2 commit | no command ownership, no sidecars | caller may retry same command |
| M2 committed, no intent name | claimed/recovery row only | classify exact claim-before-intent prefix, CAS-reacquire, then create intent |
| intent entry exists with partial/invalid record | row plus untrusted intent | fail closed; never delete; operator action |
| full 512-byte intent write returned before file/parent full-sync | same-OS restart observes a valid record, without a power-loss durability claim | parse/revalidate and continue recovery |
| valid intent durable, no staging | provable owned intent | create staging and continue |
| valid intent plus partial staging | provable owned staging | validate, unlink+sync, recreate and recopy |
| valid intent plus complete staging before M9 | provable owned staging | deep-verify; publish or recopy |
| `renameat` returned at M9, parent sync not returned | same-OS restart observes the final name and valid intent; power-loss durability is not claimed | inspect exact final; never overwrite; continue only if identity/content match |
| M10 returned, command not completed | durable final plus valid intent | deep-verify final then complete row |
| M11 committed, intent remains | completed row plus valid intent | replay success; retry safe cleanup and report `cleanup_pending` |
| M12 unlink returned, parent sync not returned | same-OS restart observes intent absent and completed row | replay success; no sidecar write |
| M12 unlink failed/did not return | completed row; valid intent may remain | replay success; retry safe cleanup and report `cleanup_pending` |
| completed row, final later absent/changed by user | stored completed result | replay result only; never recreate or overwrite |
| target existed at initial validation | no command and no sidecars | pre-claim `CONFLICT`; target untouched |
| target raced into existence before publish | no-clobber conflict; target untouched | terminal `CONFLICT` after proven cleanup, otherwise recovery-required |
| parent/device/inode/ACL replaced at any revalidation | authority lost | recovery-required; no further target mutation |
| CAS source missing/unsafe/wrong length/hash | no publish | `STORAGE_CORRUPTION`; affected custody issue |
| disk full/quota/I/O before publish with proven cleanup | no final/sidecars | terminal `STORAGE_IO_ERROR`; fresh command after condition changes |
| disk full/I/O after publish | final may exist | recovery-required; same command only |
| process panic/channel loss at any post-claim point | state not guessed | current runtime fails; durable retry matrix on restart |

SIGKILL tests assert the exact visibility of every syscall that returned in the same
running OS. Syscall failure/order tests use deterministic seams/traces. Power loss
and kernel/device durability remain `UNVERIFIABLE` unless separate hardware evidence
is added; the implementation still uses `F_FULLFSYNC` for the accepted macOS
durability contract and does not overclaim those tests.

Every recovery action after an observed prior-runtime or `RECOVERY_REQUIRED` row is
preceded by the §2.10 physical classifier and successful materialize-specific CAS
reacquire. The table never authorizes direct filesystem work from the shared ingest
`RecoveryRequired` outcome.

## 12. Observability and health

### 12.1 Structured Core logs

The application produces only a closed `CoreLogEvent` with:

```text
schema_version, timestamp_or_unavailable, severity, service, build_version,
event_kind, operation_kind, outcome_kind,
optional request_id, optional correlation_id, optional command_id,
optional causation_id, optional error_code, optional retryable,
optional duration_ms, optional issue_kind
```

The closed V1 enum registries are exact:

```text
CoreLogSeverity = ALERT | ERROR | WARN | INFO | DEBUG | TRACE
CoreLogEventKind = SERVICE_STARTING | SERVICE_READY | SERVICE_STOPPING |
  OPERATION_STARTED | OPERATION_COMPLETED | OPERATION_FAILED |
  DB_OBSERVATION | STORAGE_OBSERVATION | INTEGRITY_ISSUE_OBSERVED |
  HEALTH_TRANSITION | RECOVERY_CLASSIFICATION
CoreOperationKind = NONE | HANDSHAKE_V1 | ASSET_INGEST_COPY_V1 |
  LIBRARY_STATUS_V1 | LIBRARY_VERIFY_V1 |
  LIBRARY_INTEGRITY_ISSUES_LIST_V1 | ASSET_INSPECT_V1 | ASSET_LIST_V1 |
  ASSET_MATERIALIZE_V1 | STARTUP_LOCAL_CLASSIFICATION_V1
CoreOutcomeKind = NONE | STARTED | SUCCEEDED | REPLAYED |
  TERMINAL_REJECTED | RECOVERY_REQUIRED | CANCELLED |
  DEADLINE_EXCEEDED | BACKPRESSURE | FAILED
```

No variant has an arbitrary string fallback. Canonical acceptance fixes these Rust
enum discriminants and exact uppercase encoder strings; later tasks extend them by
reviewed variants rather than an open label. `NONE` is valid only for service,
health and unattributed startup DB/storage events; an authenticated operation event
must use its exact non-`NONE` variant.

Field applicability is closed as follows. `R` means required, `A` means required
when the event is attributable to an authenticated operation/command, and `-` means
forbidden:

| Event kind | operation | outcome | request/correlation | command | error/retryable | duration | issue |
|---|---|---|---|---|---|---|---|
| service starting/ready/stopping | `NONE` | `NONE` | - | - | - | - | - |
| operation started | R | `STARTED` | R | A | - | - | - |
| operation completed | R | `SUCCEEDED` or `REPLAYED` | R | A | - | R | - |
| operation failed | R | exact non-success outcome | R | A | R | R | - |
| DB observation | R | success/failure outcome | A | A | A on failure | R | - |
| storage observation | R | success/failure outcome | A | A | A on failure | R | - |
| integrity issue observed | `LIBRARY_VERIFY_V1` | `SUCCEEDED` | R | - | - | - | R |
| health transition | `NONE` | `NONE` | - | - | - | - | - |
| recovery classification | `STARTUP_LOCAL_CLASSIFICATION_V1` | R | - | A | A on failure | R | - |

`causation_id` is forbidden for every TASK-008 event. `request_id` and
`correlation_id` are the already authenticated session values and are both present
or both absent. `command_id` is required for ingest/materialize operation events and
per-command recovery classification, and forbidden for query/status events. An
operation emits exactly one started event and one terminal completed/failed event;
internal DB/storage observations do not increment the Core terminal-operation count.

`schema_version` is exactly 1. `service` is the compiled enum value `MENGXIAD`;
`build_version` is 1..64 bytes of validated ASCII `[A-Za-z0-9._+-]` package/build
version injected by the encoder rather than caller input.
Timestamp is one typed timestamp sampled at event creation or the closed
`UNAVAILABLE` state; clock failure cannot fabricate time or rewrite the operation
outcome. `duration_ms` is present on terminal operation/DB/storage observations,
checked against u64 and the operation deadline. `retryable` comes from the exact
operation/result mapping, never a context-free ErrorCode property. `causation_id`
is absent for current operations that have no causal predecessor; the field remains
reserved for the later accepted workflow extension. Project/Run/Provider/Plugin
identities are not applicable to TASK-008 and remain future closed-schema additions,
not an arbitrary map.

All values are enums, fixed grammar values or already validated fixed-width IDs.
There is no arbitrary message/details map. Production encoding is one ASCII line no
longer than 1024 bytes with fixed ordered `key=value` fields; absent identifiers
encode `-` and IDs encode canonical lowercase text. Newline/control/quote injection
is structurally impossible. The encoder computes the exact length before allocation,
and exhaustive enum/max-build-version/four-UUID/error/issue golden vectors prove the
closed worst case is within `CORE_LOG_LINE_BYTES_MAX=1024`; exceeding it is the
closed `encode_rejected` drop reason and marks observability degraded, never
truncation. A `Sensitive<T>` wrapper has no value accessor into the
log encoder and always renders the literal `[REDACTED]` in explicit redaction tests.

The daemon owns one bounded 256-event non-blocking queue and one joined log writer.
Queue saturation drops the event, increments a closed counter and never blocks a
Core operation. Writer failure marks observability degraded and increments the
drop/error counter; it does not rewrite a successful canonical operation or pretend
the local store is corrupt. Shutdown closes the queue and joins the writer before
releasing Library ownership. Panic/join failure is `INTERNAL_ERROR` during shutdown.

### 12.2 Metric schema

The initial in-process metric catalog is closed and versioned. It retains the exact
canonical §14.1 per-error metric names; a generic error-labeled counter must not
replace them:

```text
core_commands_total{operation,outcome}
core_command_duration_ms{operation,outcome}                 histogram

validation_failures_total, auth_failures_total, policy_denials_total,
conflicts_total, invalid_transitions_total, ingest_source_races_total,
storage_errors_total, integrity_failures_total, storage_busy_total,
storage_configuration_errors_total, ipc_transport_errors_total,
protocol_version_unsupported_total, deadline_exceeded_total,
operation_cancelled_total, provider_errors_total{class},
unknown_submissions_total, plugin_violations_total, sandbox_denials_total,
revocation_blocks_total, queue_rejections_total, internal_errors_total,
command_in_progress_total, admin_auth_unavailable_total,
unsupported_capability_total, id_generation_failures_total,
revision_exhaustion_total

db_transaction_duration_ms{kind,outcome}                    histogram
db_write_queue_depth                                       gauge
db_wal_bytes                                                gauge
storage_bytes_total{operation,direction}
storage_operation_duration_ms{operation,outcome}            histogram
storage_staging_orphan_count                                gauge
storage_staging_orphan_bytes                                gauge
library_integrity_issues_total{issue_kind}
core_health_transitions_total{readiness,degraded_class}
core_observability_dropped_total{reason}
```

Allowed labels are finite enums compiled with the catalog. Raw/dynamic label APIs
do not exist. Request/command/correlation/Asset/Run IDs, path/digest, raw error
messages and Provider-supplied strings are forbidden. TASK-008 does not add a
network exporter or numeric SLO. Throughput is derived from the closed storage-byte
counter and duration histogram; it is not a high-cardinality instantaneous label.
All duration histograms use the accepted millisecond buckets
`[1,5,10,25,50,100,250,500,1000,2500,5000,10000,60000,300000,3600000,86400000]`;
values above the final bucket increment a closed overflow bucket rather than growing
the schema. Counters are checked `u64`: the first attempted
overflow pins that counter at `u64::MAX`, marks observability degraded and records
one closed saturation reason without recursion. A deterministic near-maximum seam
must test this boundary; wraparound and panic are forbidden.

The exact V1 label registries are:

```text
operation = asset.ingest.v1 | library.status.v1 | library.verify.v1 |
  library.integrity-issues.list.v1 | asset.inspect.v1 | asset.list.v1 |
  asset.materialize.v1
core outcome = succeeded | replayed | terminal_rejected | recovery_required |
  cancelled | deadline_exceeded | backpressure | failed
db kind = startup_check | query_page | verify_page | command_observe |
  command_claim | command_reacquire | command_complete | command_disposition |
  recovery_classify
db outcome = read_succeeded | committed | rolled_back | busy | interrupted | failed
storage operation = cas_read | verify_metadata | verify_hash | materialize_copy |
  materialize_sync | materialize_publish | materialize_cleanup
storage outcome = succeeded | existing_verified | cancelled | deadline_exceeded |
  configuration_error | io_error | corruption | failed
direction = read | write
degraded_class = none | custody_read_only | custody_blob | dependency |
  observability
drop reason = queue_full | writer_failed | counter_saturated | encode_rejected
provider class = unavailable | authentication | rate_limited | transient |
  permanent | malformed | unknown_redacted
```

`issue_kind` is exactly the fourteen `IntegrityIssueKind` non-zero values. `readiness`
is exactly `not_ready|ready`. Provider labels are reserved/compiled for canonical
compatibility but no TASK-008 production path increments `provider_errors_total`.
No label accepts `str`, numeric ID or externally supplied enum text.

Update sites are also fixed: one terminal API disposition increments
`core_commands_total`, its duration histogram and exactly one canonical §14.1
per-error counter when unsuccessful; one DB job updates the DB duration and outcome,
with queue depth sampled on admission/release and WAL bytes after startup checks and
successful writes; storage byte counters add bytes actually read/written at checked
chunk boundaries and one terminal storage action updates its duration; staging
gauges replace their values only after a complete bounded observation; every
discovered integrity issue, including one not retained after the 4096 cap, increments
its issue-kind counter; health transitions increment only on an actual typed state
change; and each dropped/unencodable event increments exactly one drop reason.
Metric-update failure cannot recursively emit another log event.

### 12.3 Distinct health semantics

`GetLibraryStatus` returns independently typed fields:

- liveness: `LIVE`, `STOPPING`, `FAILED`;
- readiness: `NOT_READY`, `READY`;
- availability: `FULL`, `READ_ONLY_CUSTODY`, `DEGRADED_CUSTODY`,
  `DEGRADED_DEPENDENCY`;
- local security baseline: `VERIFIED`, `UNSAFE`, `UNAVAILABLE`;
- custody observation: `UNASSESSED`, `NORMAL_VERIFIED`, `DEEP_VERIFIED`;
- readiness block reason: `NONE`, `LOCAL_RECOVERY_PENDING` or
  `LOCAL_RECOVERY_OPERATOR_ACTION`;
- fixed capability booleans for metadata read, verify, ingest and materialize;
- the fixed typed availability/capability and observation fields in the protocol
  schema, never an open-ended degraded-reason string or map;
- staging orphan count/bytes and current backend-match boolean;
- no path, UID, backend ID, locator, error detail or secret.

Liveness says only that the daemon event loop can answer. Readiness requires local
Library ownership, SQLite invariants, an operational writer and the §4.1 captured
local-mutation classification. The bound endpoint remains `NOT_READY` and
status-only before those gates pass. `READ_ONLY_CUSTODY` permits status,
Inspect/List and Verify but forbids ingest/materialize. A future Provider outage is
`DEGRADED_DEPENDENCY` and does not remove unrelated local capabilities, satisfying
the TASK-008 foundation portion of AC-015. The full Provider E2E obligation remains
with its future owning runtime task.

`READY` requires block reason `NONE`; `NOT_READY` requires one of the two non-none
reasons. While classification is progressing, non-status operations return
pre-effect `BACKPRESSURE` with bounded fresh admission guidance. A durably
unclassifiable but safely reportable local recovery condition returns static
`STORAGE_CONFIGURATION_ERROR` and retains status-only operation; corruption or
lost Library authority follows the fatal shutdown rules instead of masquerading as
ordinary not-ready state.

`recovery_observation_available=false` until both S7 passes complete; while false,
`recovery_required_command_count` is exactly zero and is not a partial count. On
successful completion it atomically becomes true with the exact validated count at
the captured boundary. A later exact command recovery updates that in-memory count
only after its durable transaction commits. On any `NOT_READY` row all four product
capability booleans are false regardless of the provisional availability class; the
table below applies only after readiness is `READY`.

The availability priority and capability truth table is fixed:

| State | read metadata | verify | ingest | materialize |
|---|---:|---:|---:|---:|
| `FULL` | yes | yes | yes | yes |
| `READ_ONLY_CUSTODY` (backend/global namespace mismatch) | yes | normal diagnostic yes | no | no |
| `DEGRADED_CUSTODY` (isolated Blob issue) | yes | yes | yes | yes, with affected Member rejected |
| `DEGRADED_DEPENDENCY` (future Provider only) | yes | yes | yes | yes |

When several conditions coexist, `READ_ONLY_CUSTODY` outranks
`DEGRADED_CUSTODY`, which outranks `DEGRADED_DEPENDENCY`, which outranks `FULL`.
Fatal local state closes admission and transitions to shutdown rather than entering
this table. Capability booleans must match the selected row; inconsistent
construction is `INTERNAL_ERROR`.

`FULL` means that all declared current-runtime capabilities are available; it is
not a claim that every Blob body was recently hashed. Startup sets custody
observation to `UNASSESSED` because it neither enumerates nor hashes all canonical
Blobs. A completed NORMAL/DEEP report changes only the in-memory observation and may
move availability to `DEGRADED_CUSTODY`; a materialize-time safety failure may do
the same. Restart returns the observation to `UNASSESSED`. TASK-008 deliberately
does not update Location lifecycle or `verified_at`, and no persisted custody-health
claim is made. Any independently durable health history requires a later explicit
schema/decision gate; no existing later task is silently assigned that mutation.

The local security baseline summarizes the already executed startup Library,
endpoint and Blob-root authority checks. Destination authority remains
operation-specific and cannot be generalized to an unexamined path. This field is
not the Admin `SecurityDoctor`
operation, does not audit Plugins/Credentials/network policy and cannot be used as
authorization. TASK-013/023 retain that command and must extend these types rather
than alias liveness/readiness to “secure”.

## 13. Configuration and finite limits

The production composition root retains
`CLI > environment > Library config > compiled default`, captures each source once
and validates all selected values before endpoint publication. Store/storage/app do
not read environment variables. TASK-008 consumes the already canonical
`MENGXIA_LOG_LEVEL` and adds exactly two new config keys. All three have matching
daemon flags:

```text
MENGXIA_LOG_LEVEL
  daemon flag: --log-level
  default: info
  accepted exact lowercase ASCII: error|warn|info|debug|trace
  ALERT events are treated as above ERROR and can never be filtered out

MENGXIA_MAX_VERIFY_OPERATION_TIMEOUT_MS
  daemon flag: --max-verify-operation-timeout-ms
  default/max: 86400000 (24 h)
  accepted: 100..86400000, tightening-only

MENGXIA_MAX_MATERIALIZE_OPERATION_TIMEOUT_MS
  daemon flag: --max-materialize-operation-timeout-ms
  default/max: 86400000 (24 h)
  accepted: 100..86400000, tightening-only
```

Log filtering occurs on typed severity before queue admission; it cannot inspect or
parse a rendered message. Invalid case/value, duplicate source or non-Unicode input
fails at S0. The existing key plus the two new keys and their exact source/default/
range contracts must be synchronized into Specification §16. The fixed correctness/
abuse constants below remain non-configurable and are versioned in ADR-0011,
following ADR-0005's fixed-bound precedent; they are not hidden config.

The existing client `MENGXIA_CLIENT_OPERATION_TIMEOUT_MS`/flag supplies the
requested timeout for verify/materialize and remains 100..86400000. The server
checks it against the operation-specific ceiling. Issues/Inspect/List requests use
the same 100..86400000 hard range; their SQL/report work is additionally bounded by
one page, the 256-event scan window, the 65-row current-Blob Location budget or the
hierarchical output-row budget, the DB busy budget and the existing non-queued read
slot. They need no additional server
ceiling: the client deadline bounds delivery while the accepted row/query budgets
independently bound work. Status is an in-memory snapshot under the existing
transport deadline. Invalid selected higher values,
empty/whitespace/sign/non-ASCII/non-Unicode numeric input, zero, underflow,
overflow, ceiling expansion and unknown config keys fail before endpoint mutation.

Fixed safety constants, versioned by ADR-0011, are:

```text
QUERY_PAGE_DEFAULT                32
QUERY_PAGE_MAX                    64
STORE_SCAN_PAGE_MAX               256
INSPECT_SELECTS_PER_PAGE_MAX       512
INSPECT_LOCATION_ROWS_PER_PAGE_MAX  65
COMPLETED_VERIFY_REPORTS_MAX      4
VERIFY_ISSUES_STORED_MAX          4096
CONCURRENT_VERIFICATIONS_MAX      1
OBSERVABILITY_QUEUE_MAX           256
CORE_LOG_LINE_BYTES_MAX           1024
MATERIALIZE_INTENT_BYTES          512
MATERIALIZE_RESERVED_NAME_COUNT   2 per active command
STARTUP_LOCAL_CLASSIFICATION_TIMEOUT_MS 300000
```

They are safety caps, not performance SLOs. A later task may tighten them through a
reviewed config version but cannot silently enlarge them. Materialize execution
shares the existing TASK-007 global local-I/O execution permit so ingest plus
materialize cannot exceed accepted storage/hash concurrency. Query admission uses
the existing non-queued read slots. No hidden waiter queue is introduced.

## 14. Errors, retry and security disposition

No new ErrorCode is needed. Exact mappings are:

| Condition | Error | Retry/action |
|---|---|---|
| malformed ID/path/cursor/page/mode/timeout | `VALIDATION_ERROR` | correct input |
| syntactically valid missing object/report | `NOT_FOUND` | no automatic retry |
| cursor Asset/current-Member Blob revision or command binding changed | `CONFLICT` | restart read or use correct command |
| read/verify/materialize admission full | `BACKPRESSURE` | fresh bounded admission |
| SQLite BUSY after accepted budget | `STORAGE_BUSY` | fresh bounded admission |
| database/schema/event/graph or Blob digest integrity failure | `STORAGE_CORRUPTION` | no automatic retry |
| unsafe backend/destination/namespace/intent | `STORAGE_CONFIGURATION_ERROR` | operator/config action |
| certain filesystem failure | `STORAGE_IO_ERROR` | only after condition changes and exact retry class |
| same materialize command currently active | `COMMAND_IN_PROGRESS` | same command after bounded delay |
| deadline before materialize publish/verify completion | `DEADLINE_EXCEEDED` | verification fresh request; materialize durable matrix |
| cancellation before materialize publish | `OPERATION_CANCELLED` | fresh materialize command only after terminal cleanup |
| ID/clock generation failure | `ID_GENERATION_UNAVAILABLE` | after platform condition changes |
| panic/join/impossible variant | `INTERNAL_ERROR` | current-runtime automatic retry forbidden |

Every public display is static. Error details contain at most a safe object type/ID,
verification/issue ID or retry action permitted by the existing envelope. They
never expose existence across a different owner because peer authentication occurs
before the operation frame.

Security review must explicitly prove:

- protocol, cursor, path, DB row and filesystem inputs are untrusted and bounded;
- ordinary local owner authorization is server-derived; no Admin/tenant claim;
- destination capability cannot escape or overwrite and CAS path stays opaque;
- intent cleanup authority is cryptographically/checksum and identity bound;
- secret-like values and content/path data cannot enter logs/metrics/errors;
- all queues, reports, rows, frames, buffers, files and time are bounded;
- no retry crosses an uncertain external effect without durable state;
- no automatic delete/rebind/repair or lifecycle mutation exists;
- local fatal and dependency-degraded states cannot be confused.

## 15. Candidate acceptance and test registry

The unused canonical domain/runtime IDs AC-017 through AC-019 are proposed for this
task. TASK-008 supplies prerequisite local-first health evidence for AC-015 but must
not list or mark the complete AC-015 as its acceptance: durable/queryable affected
Runs remain owned by TASK-015. These candidate definitions are not stable until
canonical acceptance.

```gherkin
AC-017 (candidate)
Given a Library with valid, missing, unsafe, truncated or digest-mismatched custody
When normal or explicit deep verification runs
Then it returns bounded typed issues with the exact severity/availability class
And startup never hashes all Blob bytes
And no canonical row, Blob or orphan is mutated
And readiness requires the writer plus classified local mutation recovery
And Provider degradation never relabels a fatal local invariant.

AC-018 (candidate)
Given Assets with zero, one or many Blob Locations and concurrent later commits
When a Client lists Assets or inspects one selected revision across pages
Then every response is bounded by 64 items
And opaque cursors preserve documented Asset and per-Member Blob-revision snapshot/keyset semantics
And no Member/Location projection is collapsed, duplicated, omitted or mixed across an Asset revision change.

AC-019 (candidate)
Given one managed Asset member and an authorized absent destination
When MaterializeAsset succeeds, crashes or is replayed
Then exactly one no-clobber durable file contains the verified Blob bytes
And the CommandRecord deterministically completes or requires exact recovery
And no CAS root/locator or unrelated destination is exposed or modified.
```

Mandatory test IDs proposed for canonical synchronization:

| Test | Mandatory evidence |
|---|---|
| `TEST-PROTO-008` | stable exact TASK-007 1.1 source/descriptor/provenance fixture and current-schema compatibility; exact current 1.2 tags/reservations/source/descriptor/provenance/depth/preflight |
| `TEST-CLI-008` | exact status/verify/issues/inspect/list/materialize grammar, cursor/output/exit/redaction |
| `TEST-CONFIG-008` | existing log level plus two new ceilings, four-layer resolution/filtering and every invalid boundary before mutation |
| `TEST-AUTH-008` | real peer UID, absent actor/Admin/tenant fields and second-UID denial retained |
| `TEST-CURSOR-008` | exact 80/208/96-byte formats, format-1/schema-0001 binding, signed-SQLite sequence range/order, request/cursor identity rules, List last-examined/snapshot existence and no-progress rejection, Inspect last-Location primary-key revalidation and golden/checksum/library/operation/corruption vectors |
| `TEST-QUERY-008` | bounded Inspect/List positive/malformed graph/not-found/current-revision; List allocator/max endpoint and snapshot point-lookup plans; zero/one/many and multi-backend Location projections; exact Location primary-key recovery plus first/continued `locations_blob_backend_idx` plans; negative rejection of current-backend-only, `blob_digest + ORDER BY location_id`, temp-sort and unbounded-residual shapes; hierarchical maximum-graph O(page) behavior |
| `TEST-PAGINATION-008` | exact format-1 Asset creation predicate; empty database, allocator ahead/behind maximum, missing last/snapshot event and interior sequence gap; sparse/empty and dense 256-event List windows including more qualifying Assets than `page_size`, stop-at-page-full cursor advancement, no unchanged cursor/empty-page loop and no skip/duplicate; per-Blob `(backend_id,location_id)` Location traversal with no empty Inspect continuation, no collapse/omission across backends, Blob-revision membership conflict, missing/mismatched cursor Location rejection, hierarchical keysets and deterministic concurrent writes |
| `TEST-VERIFY-008` | normal versus deep byte-read proof, typed issues, report cap/eviction/truncation |
| `TEST-CORRUPTION-008` | DB/schema/event, allocator-ahead/behind, last/snapshot loss and interior sequence gap, backend/missing/unsafe/length/hash/orphan/canonical-extra matrix |
| `TEST-DESTINATION-008` | whole-prefix APFS/UID/mode/ACL/no-follow/no-clobber/replacement/non-Unicode matrix |
| `TEST-MATERIALIZE-008` | exact selection/digest/stream/full-sync/result/replay, completed replay with present/removed/changed final, zero-length Blob, empty logical name and source unchanged |
| `TEST-RECOVERY-008` | every §11 SIGKILL prefix, startup durable prior-runtime CLAIMED classification and deterministic syscall-order/failure point |
| `TEST-CANCEL-008` | owned SQLite interrupt cause/race/connection replacement plus verify and pre/post-publish deadline/disconnect/shutdown; no detached effect |
| `TEST-OBSERVABILITY-008` | exact log/event/operation/outcome/field matrix, canonical fields/error metrics, every label/update site, durations/gauges, log-level filtering, canary redaction, bounded queue/drop and closed labels |
| `TEST-HEALTH-008` | NOT_READY/READY writer+durable recovery gate, 300000-ms timeout/operator-restart path, read-only/dependency/custody/security/UNASSESSED truth table and AC-015 prerequisite seam |
| `TEST-ERROR-008` | exact code/static message/retry/runtime-effect mapping and non-disclosure canaries |
| `TEST-LIFECYCLE-008` | all workers/reports/sessions joined and Library lock released last under failure |
| `TEST-ARCH-008` | exact dependency/public/file/migration/opaque-CAS/no-Admin/no-TASK-009 boundary |
| `TEST-SUPPLY-008` | locked/offline/no-new-dependency and retained FFI/toolchain/advisory policy |
| `TEST-DOC-008` | proposal/ADR/spec/plan/requirements/AC/TEST/lifecycle/authority agreement |
| `TEST-ENDTOEND-008` | real CLI ingest -> list -> inspect -> materialize -> verify/status/replay on APFS |

Evidence mapping:

| Acceptance | Required evidence |
|---|---|
| candidate AC-017 | VERIFY, CORRUPTION, HEALTH, ERROR |
| candidate AC-018 | QUERY, CURSOR, PAGINATION, PROTO, CLI |
| candidate AC-019 | DESTINATION, MATERIALIZE, RECOVERY, CANCEL, AUTH, ENDTOEND |
| OPS-001/002/003/004 | OBSERVABILITY, HEALTH, ERROR, ARCH |

`TEST-HEALTH-008` records only prerequisite evidence against AC-015 and cannot emit
`AC-015: PASS`; the complete AC remains mapped to TASK-015.

Formal large-file evidence reuses the retained TASK-005 streaming proof and adds
materialize/deep-verify O(buffer) evidence at 1 GiB plus deterministic synthetic
length/offset boundaries. It must not duplicate the retained 10/100 GiB suites
unless implementation changes the shared streaming primitive. Real APFS, SIGKILL
and second-UID evidence remains formal-only; unit/developer tests use bounded fakes.

## 16. Canonical traceability correction proposed

TASK-008 directly consumes more requirements than its current short Plan row. On
acceptance, synchronize this exact set rather than claiming only transitive coverage:

```text
REVIEW-CONFLICT-024 (candidate): materialize-specific observe/CAS-reacquire/replay;
                                 completed ingest and revision replay unchanged
REVIEW-CONFLICT-025 (candidate): bounded 256-event List and per-Member Location
                                 windows plus hierarchical Inspect; no migration/index change
REVIEW-CONFLICT-026 (candidate): endpoint-bound NOT_READY, bounded durable prior-
                                 runtime claim classification and READY semantics
REVIEW-CONFLICT-027 (candidate): §14.1/§15.1/§15.2 exact Core log/event/label/update refinement
REVIEW-CONFLICT-028 (candidate): §10.2 exact six operation IDs/selectors/auth/results
REVIEW-CONFLICT-029 (candidate): TASK-008 supplies AC-015 prerequisite evidence only;
                                 complete AC-015 remains TASK-015
REVIEW-CONFLICT-030 (candidate): retained TEST-PROTO-007 owns immutable exact 1.1
                                 fixtures; TASK-008 owns the advancing current 1.2 artifact
REVIEW-CONFLICT-031 (candidate): List snapshot requires allocator/max equality,
                                 last/snapshot existence, contiguous traversal and
                                 fail-closed cursor progress
```

These IDs become stable only if added as canonical `DECISIONS.md` definitions during
acceptance. Canonical synchronization must also update Specification §10.2,
§13.5, §14.1, §15.1–§15.4 and §16; the Plan TASK-008/TASK-015 rows; Review ownership;
and Intake/file/evidence state. The accepted AC definitions must be exact bare
`AC-017`/`AC-018`/`AC-019` Gherkin blocks, and all twenty-one TEST IDs must be exact
Specification table definitions so the current mechanical traceability collector
can recognize them.

```text
FEATURES: FUNC-001, FUNC-003, FUNC-010
REQUIREMENTS:
  REQ-001, REQ-010, REQ-011, REQ-013,
  DATA-002, DATA-003, DATA-004, DATA-009, DATA-013,
  API-001, API-002, API-003, API-008, API-010, API-011,
  SEC-005, SEC-012, SEC-013, SEC-017, SEC-020, SEC-021,
  REL-001, REL-004, REL-005, REL-006, REL-008,
  OPS-001, OPS-002, OPS-003, OPS-004,
  CFG-001, CFG-003
DECISIONS:
  BASE-001, BASE-002, BASE-003, BASE-004, BASE-007, BASE-010,
  BASE-011, BASE-012, BASE-013, BASE-014, BASE-016, BASE-017,
  BASE-018, BASE-019,
  DEC-016, DEC-017, DEC-019, DEC-020, DEC-021, DEC-022,
  REVIEW-CONFLICT-021, REVIEW-CONFLICT-022, REVIEW-CONFLICT-023,
  REVIEW-CONFLICT-024, REVIEW-CONFLICT-025, REVIEW-CONFLICT-026,
  REVIEW-CONFLICT-027, REVIEW-CONFLICT-028, REVIEW-CONFLICT-029,
  REVIEW-CONFLICT-030, REVIEW-CONFLICT-031,
  ADR-0002, ADR-0004, ADR-0005, ADR-0007, ADR-0008, ADR-0009,
  ADR-0010, ADR-0011
PREREQUISITES: TASK-003 DONE; TASK-004 DONE; TASK-005 DONE;
               TASK-006 DONE; TASK-007 DONE
```

`SEC-008` and `SEC-019` remain TASK-013; `SEC-018` and OQ-008 remain TASK-022.
`REVIEW-GAP-005` blocks TASK-009, not TASK-008, because this design reuses the
existing `ASSET_REVISION` outcome and does not change the result union.

## 17. Implementation order after acceptance

```text
STEP-1  Accept ADR-0011 and synchronize canonical start/file-scope records; add
        AC-017/018/019 as exact bare-line Gherkin definitions and all twenty-one
        TEST IDs as canonical Specification table rows required by traceability.
STEP-2  Freeze the reviewed protocol 1.1 source/descriptor/provenance fixtures and
        retained TEST-PROTO-007 semantics, then add the current protocol 1.2
        descriptor/provenance while retaining 1.0/1.1 compatibility.
STEP-3  Add typed query/cursor/read ports and SQLite bounded read-worker jobs.
STEP-4  Add verification scanner, safe issue registry and bounded report owner.
STEP-5  Add Core typed logs, closed metrics and health state constructor/sink.
STEP-6  Add destination authority, exact intent parser/writer and managed CAS reader.
STEP-7  Add materialize command claim/recovery/orchestration and thin daemon/CLI mapping.
STEP-8  Add all fault, crash, pagination, redaction, health and real APFS E2E evidence.
STEP-9  Run TASK-008 developer gate and complete full diff/security/acceptance review.
STEP-10 Commit, push and obtain reviewed macos-26 formal plus second-UID evidence.
STEP-11 Mark TASK-008 DONE and revoke authority to NONE only after evidence is recorded.
```

No step automatically enters TASK-009.

## 18. Candidate start record

This block is the accepted Plan start record after ADR-0011, canonical
synchronization and the user's explicit conditional authorization:

```text
TASK008_CANONICAL_GATE: ACCEPTED
TASK008_LIFECYCLE: IN_PROGRESS
TASK008_IMPLEMENTATION_AUTHORITY: TASK_008_ONLY

SCOPE: TASK-008 ONLY — bounded Library verification/issues/status, Asset inspect/list,
       one-member no-clobber materialization and Core observability/health baseline.
FEATURES: FUNC-001, FUNC-003, FUNC-010
REQUIREMENTS: proposal §16 exact set
PREREQUISITES: TASK-003/004/005/006/007 DONE
DECISIONS: proposal §16 exact set including accepted ADR-0011
ACCEPTANCE: AC-017, AC-018, AC-019
AC-015_EVIDENCE: prerequisite local-first health seam only; final PASS owned TASK-015
TESTS: proposal §15 exact twenty-one TEST IDs
DEVELOPER_GATE: scripts/verify-task-008.sh developer
FORMAL_COMPLETION_GATE: scripts/verify-task-008.sh formal
AUTHORIZED_FILES: proposal §3 exact list and restrictions
FORBIDDEN: proposal §3.1; migrations/root rebind/deletion/Admin/TASK-009+ remain unauthorized
```

## 19. Independent review checklist

The gate remains blocked unless every answer is yes:

- Does the plan preserve migration 0001 and leave migration 0002 to TASK-009?
- Do ListAssets and Inspect use the §2.1/§2.2 existing-index bounded plans with no
  temp sort, unbounded residual scan or hidden index request, including primary-key
  cursor recovery and the current-Blob compound Location keyset?
- Does ListAssets prove allocator/max equality at snapshot capture, revalidate the
  last/snapshot endpoints and contiguous traversal, and fail closed rather than emit
  a non-progressing cursor?
- Does retained TEST-PROTO-007 verify immutable exact protocol 1.1 fixtures while
  TEST-PROTO-008 alone owns the advancing current 1.2 artifact?
- Is every API-010 dimension explicit for all six operations?
- Do all three cursors have exact format, snapshot, ordering, concurrency and expiry semantics?
- Can InspectAsset return zero/one/many valid Locations without collapsing the graph,
  exposing backend identity or using an unbounded frame/scan?
- Is startup independent of total Blob bytes and is deep hashing explicit?
- Are reports, issue memory, scans, queues, workers, pages, frames, bytes and time finite?
- Does every issue distinguish fatal local, custody-degraded and operator-action state?
- Can a changed backend be diagnosed without rebind or mutation?
- Can any ordinary request delete/adopt/register an orphan or canonical extra Blob?
- Is the materialize request digest complete and its result representable by migration 0001?
- Are external ingest claim/replay types and ADR-0009 behavior byte-for-byte
  unchanged while materialize uses its own observe/CAS/replay ports?
- Does canonical §10.2 retain `asset.materialize.v1` while refining its selector and
  CommandRecord-backed exact-request replay-result response exactly as §2.9?
- Does the intent have an exact byte layout and sole cleanup/recovery authority?
- Is every pre/post-publish crash, cancellation, cleanup and replay outcome closed?
- Can a completed materialize command replay when its final is present, absent or
  changed without entering NEW destination validation or recreating the file?
- Can destination traversal, replacement, ACL, link, mount or case races escape/no-clobber?
- Can any proto/result/error/log/metric reveal CAS root, locator, path or secret-like data?
- Are local fatal invariants impossible to relabel as Provider degradation?
- Does NOT_READY persist until writer and captured local mutation classification pass,
  and is every prior-runtime CLAIMED row durably transitioned before READY?
- Does the fixed startup-classification deadline have a finite operator restart path
  with committed-batch progress and no automatic infinite retry?
- Can blocking SQLite work be interrupted with an owned cause, typed mapping, joined
  completion and replacement rather than reuse of an interrupted connection?
- Does the AC-015 fake seam avoid claiming a Provider implementation or complete
  AC-015 PASS exists?
- Are log and metric schemas production-consumed rather than test-only declarations?
- Do logs/metrics disposition every applicable canonical §14.1/§15.1/§15.2 item and
  implement the exact event/field/label/update registries and `MENGXIA_LOG_LEVEL` contract?
- Are all workers joined and the Library owner/lock released last?
- Is there no new dependency, unsafe/FFI surface, generic CRUD or future-task behavior?
- Do the exact file scope and canonical authority prevent implementation before acceptance?

Correction evidence for v0.2.4 was checked against canonical Specification v1.1.29,
immutable migrations 0000/0001 and repository head
`4fabe064825d79706740811e341698121c4e96d6`. The workspace's actually bundled SQLite
3.53.4, Source ID
`2026-07-24 19:02:57 bf7c7f30031888f4e796e429ab3978879485813aaca6f641c7b33e4e09459bcc`,
proves that the bounded List window uses the unique commit-sequence index, the first
Location seek and continued row-value seek use `locations_blob_backend_idx`, cursor
key recovery uses the Location primary key, and none uses a temporary sort. The
deterministic multi-backend fixture traverses without omission or duplication. The
schema-supported event maximum and snapshot-existence shapes use unique-index
endpoint/point lookups; `TEST-QUERY-008` must freeze those plans against the bundled
runtime. Allocator mismatch, a missing cursor endpoint and an interior/tail gap now
fail without cursor fabrication. The retained protocol file scope now permits
freezing exact 1.1 fixtures before advancing the current artifact to 1.2, so
TEST-PROTO-007 remains
semantically stable instead of inheriting the 1.2 hashes. The
repository documentation gate passes all nine tests. These are correction inputs
for the next independent review, not self-approval, implementation authority or
completion evidence.

## 20. Current next action

```text
READINESS: READY_TO_IMPLEMENT_TASK_008
BLOCKER: NONE
CODEX_SAFE_ACTION_NOW: execute accepted proposal STEP-2 through STEP-10 only
PRODUCTION_CODE_AUTHORITY: TASK_008_ONLY
```

Independent review of v0.2.4 passed on 2026-09-04. Version 0.2.5 records only that
acceptance and the synchronized `TASK_008_ONLY` authority. Production implementation
may begin at STEP-2 after the canonical documentation gate passes. Migration,
root-rebind, Admin, Provider/Plugin and TASK-009+ work remain forbidden.

## 21. Formal completion evidence

```text
STATUS: NOT_YET_AVAILABLE
REASON: TASK-008 is DRAFT/BLOCKED and has no implementation authority
REQUIRED_BEFORE_DONE: exact implementation commits, local developer/formal gates,
                      reviewed macos-26 aggregate and real second-UID evidence,
                      AC-017/018/019 plus applicable security verification,
                      full diff/scope/regression review, unexecuted tests = NONE
```

This placeholder is not completion evidence and cannot be changed to PASS from a
local-only run. The future TASK-008 verifier and document traceability gate must
require this exact heading before allowing `DONE`, following the TASK-006/007
precedent.
