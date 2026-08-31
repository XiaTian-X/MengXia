---
title: "TASK-007 copy-only ingest start-gate proposal"
project: "梦夏 / MengXia"
document_role: "Accepted TASK-007 implementation supplement"
status: "ACCEPTED_IN_PROGRESS"
version: "0.1.3"
date: "2026-08-30"
canonical_specification_reviewed: "IMPLEMENTATION_SPEC.md v1.1.24"
---

# TASK-007 Gate Proposal

## 0. Gate verdict

`TASK-007` is the accepted exact implementation supplement for the active
`TASK_007_ONLY` slice. It closes the transport, request, orchestration,
cancellation, idempotency, configuration and test-contract gaps found in the
current repository. The §2.5 canonical correction is accepted: cross-instance
Blob-root rebinding cannot honestly fit this copy-ingest slice or the immutable
`0001_library_assets` result contract. TASK-007 proves same-inode moves and fails
closed on a changed backend; TASK-008 may verify and report affected custody, but no
task may mutate a binding until OQ-010 is resolved and an explicit Admin-gated
command is accepted.

```text
TASK007_CANONICAL_GATE: ACCEPTED
TASK007_LIFECYCLE: IN_PROGRESS
TASK007_IMPLEMENTATION_AUTHORITY: TASK_007_ONLY
TASK007_PROPOSAL_VERSION: 0.1.3
```

This supplement and the synchronized canonical start record authorize only the §3
file scope. They do not authorize a migration, root rebinding, source deletion,
adopt/reference custody, Project/Admin behavior, TCP,
Plugin/Provider/Credential/Rights work, GC or any TASK-008+ implementation.

## 1. Inputs, repository evidence and prerequisites

The candidate was derived in authority order from:

1. `docs/spec/IMPLEMENTATION_SPEC.md` v1.1.24;
2. `docs/spec/DECISIONS.md` and accepted ADR-0001 through ADR-0008;
3. `docs/spec/IMPLEMENTATION_REVIEW.md`;
4. `docs/spec/IMPLEMENTATION_PLAN.md`;
5. `docs/spec/PROJECT_INTAKE_REPORT.md`;
6. accepted TASK-003, TASK-005 and TASK-006 proposals and their completed code;
7. the clean `main` repository at commit `5c1627b`.

| Prerequisite | Concrete evidence | Result |
|---|---|---|
| TASK-003 complete | protected UDS, server-derived owner principal, bounded handshake and reviewed second-UID CI | PASS |
| TASK-005 complete | opaque source/root capabilities, bounded local CAS and reviewed formal CI | PASS |
| TASK-006 complete | immutable 0001, command ledger, event transaction and reviewed run `33257331689` | PASS |
| Product ingest endpoint | proto contains only terminal handshake; CLI only accepts `handshake` | EXPECTED_GAP owned here |
| Composition | daemon opens store/endpoint but does not construct `LocalBlobStorage` or app ingest service | EXPECTED_GAP owned here |
| Current authority | synchronized AGENTS/Spec/Plan/Review/Intake say `TASK_007_ONLY` | PASS / exact §3 scope only |
| Activation baseline | proposal-only review commits precede the synchronized v0.1.3 start gate | PASS |

The Plan dependency text must be corrected from `TASK-003, TASK-006` to
`TASK-003, TASK-005, TASK-006`: TASK-007 directly constructs and consumes the
TASK-005 source, control, capacity and durable-Blob contracts. This is a missing
direct prerequisite, not a new feature.

## 2. Blocking discrepancies and proposed resolutions

### 2.1 GAP-007-001 — the accepted handshake is terminal

Classification: `ARCHITECTURE / EXPECTED_GAP`

`serve_handshake` writes one `HandshakeResponse`, shuts down the stream and returns
only after the channel is terminal. Extending the daemon by reading another frame
after that function would be impossible; removing shutdown would also make the
existing `mengxia handshake` client wait ambiguously and would alter completed
TASK-003 behavior.

Resolution: retain the exact terminal handshake APIs and add a compatible
single-command session API. `ClientHello` receives an enum field at tag 6:

```proto
enum ClientIntent {
  CLIENT_INTENT_HANDSHAKE_ONLY = 0;
  CLIENT_INTENT_SINGLE_COMMAND = 1;
}

// ClientHello additions only.
ClientIntent intent = 6;
reserved 7 to 15;
reserved "admin", "credential";
```

The existing tag 3 and names `actor`/`actor_principal` remain reserved. Zero/default
intent preserves the completed handshake-only behavior. Protocol 1.0 remains the
exact terminal-handshake contract; single-command ingest is the additive protocol
1.1 capability. An ingest client sends `min_protocol_minor=max_protocol_minor=1`.
The retained `request_handshake` always emits `HANDSHAKE_ONLY` and the retained
`serve_handshake` accepts only that intent and still shuts down the stream; it
rejects `SINGLE_COMMAND` rather than returning a misleading terminal success.
The daemon's new dispatcher preserves the existing handshake-only negotiation
predicate and selects 1.0 for that intent. For `SINGLE_COMMAND` it requires exact
`min_protocol_minor=max_protocol_minor=1` and selects 1.1; it does not accept a
0-through-1 range that an old server could legally downgrade to terminal 1.0.
An old 1.0 server therefore returns `PROTOCOL_VERSION_UNSUPPORTED` rather than
silently closing a supposed product session, while the new server continues to
negotiate 1.0 with the unchanged handshake CLI. The daemon authenticates the peer
before reading a frame, negotiates once, and either closes immediately for
`HANDSHAKE_ONLY` or reads exactly one operation frame for `SINGLE_COMMAND`. Unknown
intent values fail with `VALIDATION_ERROR`; no fallback exists. A session accepts no
second operation, pipelining, multiplexing, TCP or Admin upgrade.

### 2.2 GAP-007-002 — the product operation has no complete wire contract

Classification: `SPECIFICATION / API`

The common envelope is illustrative and the committed descriptor is handshake-only.
The exact TASK-007 contract is the proto in §5. It defines every tag, bound,
reserved authority field, request/response/error branch and descriptor-depth root.
Media bytes never enter a frame. Committed `.proto`, descriptor set and provenance
remain descriptor-first, hash-pinned and generated offline exactly as in TASK-003.

### 2.3 GAP-007-003 — request identity and creative intent are under-specified

Classification: `DATA_MODEL / SECURITY`

`ManagedRegistrationPlan` requires five graph/Location IDs and five semantic values,
while completion additionally requires two event IDs, but
the canonical operation table only says “intended identity”. Section 6 freezes the
caller-owned semantic fields and the application-owned generated fields. No default
kind is guessed, no filename becomes a domain path, and no caller supplies Asset,
Revision, Representation, Resource, Location, event, principal or timestamp IDs.

### 2.4 GAP-007-004 — external claim, CAS admission and cancellation ordering is absent

Classification: `RELIABILITY / SECURITY`

TASK-005 exposes one atomic physical `ingest` admission rather than a reservable
filesystem permit, and TASK-006 permits a standalone claim only for
`asset.ingest.v1`. Section 8 therefore freezes a bounded two-level order:
validate/open source, compute binding, register the active command and acquire an
in-process execution permit, durably claim, then call CAS. This protects ordinary
commands from predictable default-capacity saturation without pretending to
reserve changing filesystem capacity. Any remaining physical backpressure after
claim is terminal for that command and a fresh attempt uses a new command ID,
matching the accepted TASK-006 contract. Cancellation is terminal only while
TASK-005 proves cleanup before promote; after a durable `DurableBlob` exists,
registration must finish or the runtime becomes recovery-required.

### 2.5 GAP-007-005 — root rebind ownership conflicts with the frozen slice

Classification: `CONFLICT / ARCHITECTURE / DATA_INTEGRITY`

ADR-0007 and the Specification edge-case table assign copied/recreated/cross-volume
root rebinding to TASK-007, while TASK-007 is otherwise one `asset.ingest.v1`
vertical slice. A safe rebind is a second mutation operation: it must authenticate
explicit operator intent, durably bind a command, verify every affected digest at
its unchanged locator, atomically update multiple Locations, append an event and
store a replayable result. The immutable 0001 result union has only `ASSET`,
`ASSET_REVISION` and `LOCATION`; no result can represent a multi-Location rebind.
Silently updating rows during startup would violate `DATA-009`, and partial or
unverified update would violate `DATA-002`, `DATA-003` and ADR-0007.

Minimum safe correction proposed for canonical acceptance:

- TASK-007 accepts a configured Blob root whose computed backend ID either has no
  prior Location rows or matches the existing local backend used by those rows;
- a same-device/same-inode rename continues to match and is covered here;
- a changed backend ID does not rewrite or reinterpret any Location. Existing old
  Locations remain unavailable and new ingest is disabled with
  `STORAGE_CONFIGURATION_ERROR` until explicit reconciliation;
- TASK-008 may perform bounded verification and report the affected old managed
  custody, but it cannot mutate a Location, expose Admin authority or silently gain
  ownership of `RebindLocalStorage`;
- a future task may own a separately reviewed rebind command only after OQ-010 is
  resolved, Admin authorization/elevation exists, and its result, restart and
  transaction contract are accepted. No current task owns that mutation.

This correction changes ownership, not safety semantics. It is accepted in the
synchronized Specification, Plan, Decisions, ADR-0007 and ADR-0009 start gate.

### 2.6 GAP-007-006 — production ingest configuration is not composed

Classification: `REPOSITORY / CONFIGURATION`

The daemon resolves only store/handshake values. Section 7 assigns all existing
TASK-005 `MENGXIA_*` storage keys to the daemon composition root using
`CLI > environment > Library config > compiled default`, constructs one immutable
`BlobStorageConfig` before endpoint publication, and adds one operation-deadline
ceiling. Store/storage crates continue to validate typed DTOs and never capture the
ambient environment.

### 2.7 GAP-007-007 — application seams are private but no architecture change is needed

Classification: `EXPECTED_GAP / ARCHITECTURE`

`AssetPersistenceService`, clock and identity seams are currently crate-private.
TASK-007 may add one public application service and owned request/result DTOs that
depend only on domain/ports/types. The daemon supplies `SqliteAssetStoreHandle` and
`LocalBlobStorage`; proto types remain in the daemon adapter and do not cross into
application/domain/ports. No new crate or third-party dependency is required.

### 2.8 GAP-007-008 — “Library config” is only a test DTO

Classification: `SPECIFICATION / SECURITY / CONFIGURATION`

The canonical priority requires `CLI > environment > Library config > default`, but
the repository has no production Library-config location, format, parser or secure
reader. `DaemonLibraryConfig::default()` and `ClientLibraryConfig::default()` cannot
prove a four-layer resolver. Putting an ad-hoc file inside the Library would also
violate its exact namespace, while adding a table would require an unauthorized
migration.

Resolution: §7 defines an optional, explicitly selected external owner-only config
file. Its pointer comes only from `--library-config` or
`MENGXIA_LIBRARY_CONFIG`; the file can supply the Library root and all non-secret
TASK-003/004/005/007 values. It is read descriptor-first, once, before any Library,
Blob-root or endpoint mutation. Absence means the third layer is absent, not a fake
default DTO. No config file is created or modified by MengXia.

## 3. Exact implementation scope after acceptance

Only the following paths may change:

```text
Cargo.toml                         # add only the existing local storage member to workspace dependencies
Cargo.lock                         # mechanical local/sha2 dependency-list changes only; no new package
proto/core/v1/handshake.proto
proto/core/v1/handshake.pb
proto/core/v1/handshake.provenance
crates/mengxia-core-proto/Cargo.toml
crates/mengxia-core-proto/build.rs
crates/mengxia-core-proto/src/lib.rs
crates/mengxia-core-proto/src/session.rs        # optional narrow protocol-1.1 module
crates/mengxia-app/Cargo.toml
crates/mengxia-app/src/lib.rs
crates/mengxia-app/src/asset_persistence.rs
crates/mengxia-app/src/config.rs
crates/mengxia-ports/src/lib.rs             # only if a narrow public accessor is proven necessary
crates/mengxia-storage-local/src/config.rs  # tests/accessors only; no custody semantic change
crates/mengxia-storage-local/src/lib.rs     # tests/accessors only; no CAS algorithm change
crates/mengxia-platform-fs/src/lib.rs       # narrow owner-only config reader export
crates/mengxia-platform-fs/src/config_file.rs
crates/mengxia-platform-fs/src/runtime_endpoint.rs # test-only post-publication SIGKILL ready signal; no production behavior change
crates/mengxia-store-sqlite/src/lib.rs      # composition accessor only if required
crates/mengxia-store-sqlite/src/asset_repository.rs # read-only backend-binding preflight
bins/mengxia/Cargo.toml
bins/mengxia/src/main.rs
bins/mengxia/src/config.rs                      # optional thin resolver module
bins/mengxia/src/ingest.rs                      # optional thin operation adapter
bins/mengxiad/Cargo.toml
bins/mengxiad/src/main.rs
bins/mengxiad/src/config.rs                     # optional composition resolver module
bins/mengxiad/src/ingest.rs                     # optional session/supervision adapter
crates/mengxia-testkit/Cargo.toml
crates/mengxia-testkit/tests/task_007_foundation.rs
crates/mengxia-testkit/tests/document_traceability.rs
scripts/verify-task-007.sh
.github/workflows/ci.yml             # add TASK-007 gate; retain formal macos-26 tuple
docs/proposals/TASK-007-GATE-PROPOSAL.md
docs/spec/IMPLEMENTATION_SPEC.md
docs/spec/IMPLEMENTATION_PLAN.md
docs/spec/IMPLEMENTATION_REVIEW.md
docs/spec/PROJECT_INTAKE_REPORT.md
docs/spec/DECISIONS.md
docs/spec/adr/ADR-0007-local-cas-custody-boundary.md
docs/spec/adr/ADR-0009-copy-ingest-session-and-orchestration.md
AGENTS.md
```

Conditional/optional files may be omitted, but if separation is needed the listed
modules prevent further growth of the existing 382-line CLI, 752-line daemon and
791-line protocol roots without authorizing a generic framework. No SQL or
migration file is authorized.

The dependency delta is closed and uses only already pinned packages/members:

```text
workspace.dependencies: add mengxia-storage-local = exact local path/version
mengxia-app:             add sha2.workspace = true
mengxiad:                add mengxia-domain.workspace = true
                         add mengxia-ports.workspace = true
                         add mengxia-storage-local.workspace = true
```

`sha2` is already pinned in the workspace/lock and is used by app only for the
version-frozen canonical request digest. The daemon edges are composition-only: it
constructs validated domain values, the cooperative `IngestControl`, and the local
storage adapter. No other normal/build/dev dependency, feature, default feature,
workspace member or third-party package is authorized; Cargo.lock may change only
the affected workspace-package dependency lists.

### 3.1 Explicitly forbidden

- changing `0000_store_bootstrap.sql` or `0001_library_assets.sql`;
- adding a migration or a storage-root rebind mutation;
- changing TASK-005 CAS layout, durability, capacity, source-stability or cleanup;
- changing TASK-006 command/event transaction semantics;
- exposing a path, descriptor, SQLite handle, backend root or lock authority;
- source delete/move/chmod/xattr, adopt/reference/unknown-mode fallback;
- Project fields/policy, Admin endpoint/elevation, TCP/HTTP, multiple commands per
  session, list/query/materialize/revision/retire/restore operations;
- automatic retry, detached work, unbounded queue, unbounded task or unbounded log;
- new dependency, unsafe/FFI, secret/credential or later-task behavior.

## 4. Architecture and ownership

The exact runtime flow is:

```text
mengxia CLI
  -> UDS + core-proto Client session adapter
  -> mengxiad composition/transport mapper
  -> mengxia-app IngestAssetCopyService
       -> mengxia-ports BlobStorage (TASK-005 LocalBlobStorage)
       -> mengxia-ports AssetUnitOfWork (TASK-006 SQLite handle)
  -> typed app result
  -> core-proto response
```

Ownership rules:

- CLI owns argument parsing, source-path bytes, a fresh request ID and the required
  user-supplied command ID. It never opens the source, hashes bytes or sees CAS paths.
- core-proto owns framing, negotiation, wire preflight, bounded decode/encode and
  safe error envelopes. It does not depend on app/domain/store/storage.
- daemon derives `PrincipalContext` from the authenticated peer, maps proto to app
  DTOs, constructs dependencies, supervises sessions and owns shutdown ordering.
- app owns semantic validation, canonical request digest, IDs/timestamps, claim/CAS/
  completion orchestration and outcome mapping.
- storage owns source descriptor stability, admission, streaming/hash/promote and
  opaque `DurableBlob`.
- store owns Library principal binding, command ledger, graph/events transaction and
  exact replay/conflict/recovery result.

Public app DTOs contain owned bounded semantic values and `PathBuf` only for the
untrusted source selector. They contain no prost/rusqlite/platform adapter type.

The one new store-specific composition seam is exact and read-only:

```text
SqliteAssetStoreHandle::validate_local_managed_backend(current_backend_id)
    -> AssetPortFuture<'_, ()>
```

It runs on the existing bounded writer and validates the candidate as exactly
`mengxia.local-cas.v1/` followed by 64 lowercase hexadecimal characters. It checks
only the closed local-managed backend family: zero such rows passes; rows whose one
distinct well-formed local backend equals the candidate pass; a different or
multiple local backend, or a malformed value beginning with the local-family
prefix, fails before endpoint publication and before new ingest. Rows outside the
exact local-family prefix are outside this local adapter preflight and are not
rejected merely for existing; their validity remains the responsibility of their
own provider/operation. The method returns no backend, locator, count or path.

The SQL performs two indexed existence probes, not an aggregate or a scan through
all rows belonging to the candidate backend. Under SQLite BINARY ordering the exact
family range is `["mengxia.local-cas.v1/", "mengxia.local-cas.v10")`; one probe
seeks a value in `[family_start, candidate)`, and the other seeks a value in
`(candidate, family_end)`, each with `LIMIT 1`. The immutable 0001
`UNIQUE(backend_id, locator)` index has `backend_id` as its leading column, so each
probe can seek directly. A committed `EXPLAIN QUERY PLAN` assertion must prove that
both statements use that covering index and do not scan `locations`. Rust validates
the candidate before binding and treats either returned row as a redacted mismatch.
The check and endpoint publication occur while the durable Library lock is held,
the composition root is the sole owner of the store handle, and mutation admission
has not opened; therefore no same-process writer can invalidate the result. This
seam neither updates a row nor claims that the current root contains old bytes.

### 4.1 Framework-neutral application boundary

TASK-007 does not make `mengxia-app` depend on Tokio, Prost, rusqlite or the local
adapter. It adds these owned semantic shapes (private fields plus read-only
accessors; abbreviated marker imports do not change their exact meaning):

```rust
pub struct IngestAssetCopyRequest {
    command_id: Id<Command>,
    source_path: PathBuf,
    asset_kind: AssetKind,
    content_kind: ContentKind,
    representation_purpose: RepresentationPurpose,
    resource_kind: ResourceKind,
    logical_name: LogicalName,
    expected_digest: Option<Sha256Digest>,
}

pub struct IngestAssetCopyResult {
    asset_id: Id<Asset>,
    asset_revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
    location_id: Id<Location>,
    blob_digest: Sha256Digest,
}

pub struct IngestAssetFailure {
    code: ErrorCode,
    retry: IngestRetry,
}

pub enum IngestAssetExecutionError {
    Respond(IngestAssetFailure),
    RuntimeFailed,
}

pub enum IngestRetry {
    No,
    SameCommandAfterBoundedDelay,
    FreshCommandAfterBoundedDelay,
    AfterSourceStabilizesWithSameCommand,
    AfterSourceStabilizesWithFreshCommand,
    AfterOperatorOrRuntimeAction,
}

pub struct IngestAdmissionLimits { /* bounded active-binding/execution counts */ }

pub struct IngestAssetCopyService<S: BlobStorage> { /* store, storage, ID/clock */ }

impl<S: BlobStorage> IngestAssetCopyService<S> {
    pub fn new(
        store: Arc<dyn AssetUnitOfWork>,
        storage: Arc<S>,
        admission: IngestAdmissionLimits,
    ) -> Self;

    pub async fn execute(
        &self,
        request: IngestAssetCopyRequest,
        control: Arc<dyn IngestControl>,
    ) -> Result<IngestAssetCopyResult, IngestAssetExecutionError>;
}
```

`IngestAssetCopyRequest::new` takes the eight fields shown in declaration order and
performs no lossy conversion; the domain value constructors have already validated
the semantic strings. The service constructor installs the system UUIDv7/clock
sources and the bounded in-process registry/permit counter from §7.
`IngestAdmissionLimits` is built from two already validated nonzero counts and
exposes no mutable permit. A crate-private `new_with_sources` is the only
clock/identity test seam and cannot enter a production signature. None of request,
service, opened source, claim guard or failure implements `Debug`, `Display`,
serialization or cloning that could copy path/authority/effect ownership. Result
and retry enums may be `Copy` because they contain only non-secret values.

The daemon constructs exactly one production `Arc<IngestAssetCopyService<_>>` for
the one opened Library/runtime, after store/storage startup and backend preflight but
before endpoint publication. Every session clones only that `Arc`. Constructing a
service, active-binding registry or execution-permit pool per listener task or per
session is forbidden because it would bypass AC-007 and the configured global
admission bounds. Graceful shutdown joins all session clones, then drops the sole
composition-root service owner before attempting `Arc::try_unwrap` and shutdown of
`LocalBlobStorage`.

`Respond` is returned only after no claim existed or the exact disposition commit
returned. `RuntimeFailed` carries no response payload: the daemon closes the
session and begins failed shutdown. This type-level split prevents an adapter from
accidentally serializing an uncommitted post-claim error.
`IngestAssetFailure` has no public arbitrary `(ErrorCode, IngestRetry)` constructor;
crate-private named constructors accept only the exact state/code/action matrix in
§10. An impossible or future non-exhaustive port variant becomes `RuntimeFailed`,
not a newly invented pair. The daemon has read-only accessors and core-proto
validates the encoded pair again on the client side.

The retry mapping is one-to-one:
`No → NONE`,
`SameCommandAfterBoundedDelay → SAME_COMMAND`,
`FreshCommandAfterBoundedDelay → FRESH_COMMAND`,
`AfterSourceStabilizesWithSameCommand → SOURCE_STABLE_SAME_COMMAND`,
`AfterSourceStabilizesWithFreshCommand → SOURCE_STABLE_FRESH_COMMAND`, and
`AfterOperatorOrRuntimeAction → OPERATOR_OR_RUNTIME_ACTION`. The daemon may not
derive a different action from `ErrorCode`.

Only the daemon can reach the production service and it does so after TASK-003
authentication. The store still derives the principal from durable Library owner
metadata; the app request has no principal field. `source_path` has no public
accessor and is consumed by the source opener/digest routine without Debug/Display.
The result accessors expose only typed IDs/digest. Failure contains no source error,
path or dynamic string.

TASK-007 always passes `None` as `ManagedRegistrationPlan::media_type`. The current
immutable store treats a repeated digest with different Blob media metadata as
`STORAGE_CORRUPTION`; accepting caller-selected media type would therefore let an
ordinary second ingest of valid shared bytes fail the store/runtime. Media metadata
requires a separately reviewed consistency/update policy and is not inferred from
the filename or content in this slice.

The daemon executes the complete app future inside one bounded, joined blocking
task because the accepted `BlobStorage` port is synchronous. It uses the existing
Tokio runtime handle to poll the store futures from that blocking task; it does not
create a runtime per command. Spawn failure, panic or join failure is
never assumed pre-effect. Panic/join failure produces `RuntimeFailed`, sends no
product response, closes mutation admission and never drops an armed claim as a
clean rejection. Only a dispatch failure proven to occur before the app future can
start may respond `INTERNAL_ERROR` with no claim. At most
`MENGXIA_MAX_CLIENT_SESSIONS` such tasks can exist, while TASK-005 independently
admits at most its configured ingest concurrency.

## 5. Exact protocol 1.1 extension

The first frame remains `ClientHello`; the second exists only when intent is
`SINGLE_COMMAND` under negotiated protocol 1.1. `PROTOCOL_MAJOR` remains 1 and the
existing public `PROTOCOL_MINOR` remains the legacy value 0. TASK-007 adds explicit
`SINGLE_COMMAND_PROTOCOL_MINOR = 1` and server range constants 0 through 1; it does
not repurpose the legacy constant. The existing terminal
`request_handshake`/`serve_handshake` helpers stay pinned to 1.0.

The additive server negotiation returns an opaque `ServerSessionContext` only for
the 1.1 branch. It contains the private `PrincipalContext`, canonical request ID and
server-generated correlation ID and has no caller-visible constructor. The client
counterpart returns a new opaque `NegotiatedClientSession`; it does not reuse or
change the completed terminal-only `NegotiatedHandshake` type. The handshake-only
wrappers continue to call the retained terminal path. Operation frame helpers
always apply `FrameLimit`, descriptor-derived preflight and the caller-provided
monotonic deadline; no daemon code calls `prost::Message::decode` directly on an
un-preflighted frame.

`OperationLimits::new(FrameLimit, DecodeDepth)` is a separate copyable core-proto
value and rejects depth below `TASK_007_MIN_OPERATION_DECODE_DEPTH`. It contains no
semantic timeout. The daemon and client construct both `HandshakeLimits` and
`OperationLimits` once from the same selected frame/depth configuration before
connect/listen; operation read/write helpers additionally receive an absolute
`tokio::time::Instant` deadline owned by the caller. This keeps transport bounds
explicit without widening or repurposing the completed handshake type.

```proto
enum IngestMode {
  INGEST_MODE_UNSPECIFIED = 0;
  INGEST_MODE_COPY = 1;
  INGEST_MODE_ADOPT = 2;
  INGEST_MODE_REFERENCE = 3;
}

message IngestAssetCopyRequest {
  string command_id = 1;
  bytes source_path = 2;
  IngestMode mode = 3;
  string asset_kind = 4;
  string content_kind = 5;
  string representation_purpose = 6;
  string resource_kind = 7;
  string logical_name = 8;
  optional bytes expected_sha256 = 10;
  uint64 operation_timeout_ms = 11;
  reserved 12 to 31;
  reserved 9;
  reserved "media_type";
  reserved "actor", "actor_principal", "principal", "project_id",
           "asset_id", "revision_id", "location_id", "backend_id", "locator";
}

enum RetryAction {
  RETRY_ACTION_UNSPECIFIED = 0;
  RETRY_ACTION_NONE = 1;
  RETRY_ACTION_SAME_COMMAND = 2;
  RETRY_ACTION_FRESH_COMMAND = 3;
  RETRY_ACTION_SOURCE_STABLE_SAME_COMMAND = 4;
  RETRY_ACTION_SOURCE_STABLE_FRESH_COMMAND = 5;
  RETRY_ACTION_OPERATOR_OR_RUNTIME_ACTION = 6;
}

message ErrorEnvelope {
  string code = 1;
  string safe_message = 2;
  bool retryable = 3;
  optional string correlation_id = 4;
  map<string, string> safe_details = 5;
  // Additive; absent in every protocol-1.0 handshake envelope.
  optional RetryAction retry_action = 6;
}

message IngestAssetCopyResult {
  string asset_id = 1;
  string asset_revision_id = 2;
  string representation_id = 3;
  string resource_id = 4;
  string location_id = 5;
  bytes blob_sha256 = 6;
}

message CoreRequest {
  oneof operation {
    IngestAssetCopyRequest ingest_asset_copy = 1;
  }
  reserved "actor", "actor_principal", "principal", "project_id",
           "admin", "credential";
}

message CoreResponse {
  oneof response {
    IngestAssetCopyResult ingest_asset_copy = 1;
    ErrorEnvelope error = 15;
  }
}
```

Rules:

- the request frame must be `1..=MENGXIA_MAX_FRAME_BYTES`; media bytes are never
  encoded;
- `CoreRequest`/`CoreResponse` are the sole second-frame roots. Tags 2..14 are
  deliberately unassigned, not pre-authorized; a later semantic operation requires
  its own API-010 gate and descriptor review before receiving one;
- all roots are included in the committed descriptor-derived depth table and pass
  the existing allocation-free canonical-varint/depth preflight before Prost. The
  build script emits separate `HANDSHAKE_DESCRIPTOR_MAX_DEPTH` and
  `OPERATION_DESCRIPTOR_MAX_DEPTH` constants; `TASK_003_MIN_DECODE_DEPTH` remains an
  alias only of the handshake floor, while a new
  `TASK_007_MIN_OPERATION_DECODE_DEPTH` guards operation limits.
  `DESCRIPTOR_MAX_DEPTH` may remain the attested maximum of both but may not silently
  raise the completed TASK-003 floor;
- only enum value `COPY` is accepted. `UNSPECIFIED`, `ADOPT`, `REFERENCE` and unknown
  numeric values return `VALIDATION_ERROR` before source or CAS mutation;
- `expected_sha256`, when present, is exactly 32 bytes; Blob result is exactly 32
  bytes; every ID parses and re-encodes as canonical lowercase UUIDv7 text;
- protocol-1.0 handshake envelopes must have `retry_action` absent. Protocol-1.1
  operation errors must have it present, must reject `UNSPECIFIED`/unknown values,
  and must preserve the application retry action exactly. Keeping the action on the
  existing envelope avoids adding an extra embedded-message level: the legacy
  handshake depth floor stays 3 and the operation floor is descriptor-proven. The
  existing
  `ErrorEnvelope.retryable` compatibility bit is `true` for every action except
  `NONE` and `OPERATOR_OR_RUNTIME_ACTION`. `safe_details` remains empty;
- source path is raw Unix bytes, preserving non-Unicode macOS names. It is 1..1023
  bytes, NUL-free, normalized absolute syntax and is passed once to the TASK-005
  descriptor-first opener. It is never returned, persisted or logged;
- request/response have no pagination; exactly one terminal response is written on
  every cleanly classified session. The §8 `RuntimeFailed` path is the sole
  fail-closed exception and closes without a product response;
- an absent/unknown request oneof is `VALIDATION_ERROR`; TASK-007 dispatch contains
  exactly the ingest tag and no generic CRUD/reflection fallback;
- every operation error after ServerHello carries exactly that session correlation
  ID; it never echoes a caller correlation value. Successful results need no second
  correlation field because one session has one request and one response;
- on the client, any missing/extra/malformed response, invalid result ID/digest,
  wrong correlation, static-message mismatch, code/action mismatch or unknown
  response value after request transmission is uncertain transport, not a local
  request-validation failure. The CLI reports `IPC_TRANSPORT_ERROR` with
  `SAME_COMMAND` and does not expose the malformed field;
- unknown protobuf fields remain wire-opaque under the TASK-003 preflight and are
  ignored for forward compatibility, but any known reserved authority field
  reintroduced by the schema/descriptor gate fails review.

On macOS the daemon converts `source_path` with
`std::os::unix::ffi::OsStringExt::from_vec` and performs no UTF-8 replacement,
Unicode normalization, canonicalize call or path lookup before the TASK-005 opener.
The CLI uses the inverse raw `OsStrExt::as_bytes` conversion. Round-trip byte
identity is a protocol test, including invalid UTF-8 and decomposed Unicode bytes.

## 6. Semantic request, validation and canonical digest

The accepted request fields are:

| Field | Validation | Digest participation |
|---|---|---|
| mode | exact COPY | literal `COPY_V1` |
| source selector | TASK-005 accepted normalized absolute raw bytes | exact selector digest below |
| asset kind | exact domain token, 1..64 bytes | UTF-8 bytes |
| content kind | exact domain token, 1..64 bytes | UTF-8 bytes |
| representation purpose | exact domain token, 1..64 bytes | UTF-8 bytes |
| resource kind | exact domain token, 1..64 bytes | UTF-8 bytes |
| logical name | UTF-8 0..255 bytes, no Unicode control | UTF-8 bytes, empty distinct |
| expected digest | absent or 32 bytes | presence + 32 bytes |

`request_id`, `command_id`, principal, operation timeout, generated IDs and sampled
times do not participate in the canonical semantic digest.

After `BlobStorage::open_source` accepts the same path, the source selector digest is
the already accepted TASK-006 formula:

```text
SHA-256(
  ASCII "MENGXIA_SOURCE_SELECTOR_V1" || 0x00 ||
  u16_be(path_byte_length) || path_bytes
)
```

The full request digest is:

```text
SHA-256(
  ASCII "MENGXIA_ASSET_INGEST_COPY_REQUEST_V1" || 0x00 ||
  TLV(0x01, ASCII "COPY_V1") ||
  TLV(0x02, source_selector_digest_32) ||
  TLV(0x03, asset_kind_utf8) ||
  TLV(0x04, content_kind_utf8) ||
  TLV(0x05, representation_purpose_utf8) ||
  TLV(0x06, resource_kind_utf8) ||
  TLV(0x07, logical_name_utf8) ||
  TLV(0x08, 0x00) ||
  TLV(0x09, 0x00 | (0x01 || expected_digest_32))
)
```

`TLV(tag, value)` is `u8 tag || u32_be(value_length) || value`; tags are strictly
increasing and each appears exactly once. Tag `0x08` is a fixed metadata-absence
marker reserved for this digest version; it is not a request field and prevents a
future media feature from silently aliasing TASK-007 command bindings. Golden
vectors cover empty logical name, expected-digest presence/absence, non-Unicode
path bytes and one-bit/one-field changes. This encoding is private application
behavior but version-frozen because it controls durable idempotency.

The app samples `claimed_at` before attempting a new durable claim. Replay,
in-progress, rejected and recovery outcomes generate no domain IDs. Only after
`DurableBlob` returns does it sample one `completed_at`, then generate Asset,
AssetRevision, Representation, Resource, candidate Location, DomainEvent and
ProvenanceEvent IDs and construct the TASK-006 completion DTO. The single
`completed_at` is used consistently for the completion and its facts/events. A
post-CAS clock or ID-source failure cannot be called a clean rejection: the armed
guard either durably records `RecoveryRequired` with the already sampled timestamp
or remains armed and closes current-runtime mutation if even that disposition
cannot be constructed/persisted.

## 7. Configuration and finite budgets

The daemon composition root resolves every production value once before Library,
Blob-root or endpoint mutation. Precedence is
`CLI > environment > Library config > compiled default`; an invalid selected higher
layer never falls through.

### 7.1 Production Library-config source

The exact selector is optional `--library-config ABSOLUTE_PATH` over environment
`MENGXIA_LIBRARY_CONFIG`; it is not itself readable from the selected file. The raw
path is 1..1023 bytes, absolute, normalized and NUL-free. The platform reader walks
from `/` through retained no-follow directory descriptors on local APFS with
ownership checking enabled. Every ancestor is root/eUID-owned and not group/world
writable. Ancestor ACLs may contain only the already accepted non-inheritable
deny-only entries; any allow, inheritable, unknown or malformed ACL/flag is rejected.
The final parent is eUID-owned mode 0700 with empty ACL; the final file is one
regular non-symlink link, eUID-owned mode 0600 with empty ACL. Ownership-disabled
mount, hard link, FIFO/device/socket, case mismatch or any revalidation mismatch
fails before reading configuration into the resolver.

The reader accepts at most 16,384 bytes. It snapshots metadata, allocates at most
that size, uses positional bounded reads, requires exact EOF, rechecks the retained
file and selected parent edge, then returns bytes. It never logs the pointer or
contents. The wire format is byte-preserving: header, keys and separators are
ASCII, while values are opaque bytes so parsing never performs lossy replacement:

```text
MENGXIA_LIBRARY_CONFIG_V1\n
KEY=VALUE\n
KEY=VALUE\n
```

There are 0..64 entry lines, each 3..2048 bytes; the file ends in exactly one LF.
Blank lines, comments, CR, UTF-8 BOM, NUL or ASCII-control bytes other than the
required LF delimiters, duplicate/unknown/unsorted keys and an empty value are
invalid. Keys are
sorted by ASCII bytes and are limited to the canonical non-secret variables
consumed by TASK-003/004/005/007. Values remain bounded byte strings until the
existing typed resolver selects a layer and validates the chosen value; numeric and
enum values must then be exact ASCII, while path values first convert losslessly
through `OsStringExt` and then remain subject to their already accepted typed path
contracts. In particular, `MENGXIA_LIBRARY_ROOT` and `MENGXIA_BLOB_ROOT` remain
Unicode-only exactly as TASK-004/TASK-005 require; this parser does not make a
non-Unicode root valid. Source selectors are not config values, and an endpoint may
retain non-Unicode parent components only where the existing TASK-003 endpoint
validator already permits them. Rejected raw bytes are dropped and never retained
in an error.
`MENGXIA_LIBRARY_CONFIG` itself, secrets, Admin, log, Plugin, Provider and future
keys are forbidden.

Each entry splits at its first ASCII `=`. The closed uppercase-ASCII key cannot
contain `=`; a space before `=` therefore makes an unknown key. The opaque value may
contain later `=`, non-ASCII bytes or ordinary spaces at any position, including in
a path component. Numeric/enum selected-value parsers still accept only their exact
ASCII grammar. LF/CR cannot be represented inside a value in this format. This
makes parsing deterministic without performing UTF-8 replacement or normalization,
while not rejecting a path merely because an existing typed path contract permits a
leading or trailing ordinary space.

The platform crate owns only authority/read/revalidation and returns bounded bytes.
`mengxia-app::LibraryConfigDocument` owns the pure no-I/O parser and a closed
`LibraryConfigKey` enum; it stores at most one bounded value per recognized key and
implements neither Debug, Display nor serialization. Client and daemon therefore
consume the same parsed DTO without moving filesystem policy into app code. The
recognized set is exact: Library/Blob roots, Client endpoint, frame/decode/handshake/
pending-session limits, DB writer/read/busy limits, all eight TASK-005 storage
values, client/server ingest timeouts, maximum client sessions and ingest shutdown
timeout. Adding any key requires a reviewed configuration-contract change.

```text
MENGXIA_BLOB_ROOT
MENGXIA_CLIENT_ENDPOINT
MENGXIA_CLIENT_HANDSHAKE_TIMEOUT_MS
MENGXIA_CLIENT_OPERATION_TIMEOUT_MS
MENGXIA_DB_BUSY_TIMEOUT_MS
MENGXIA_DB_READ_CONNECTIONS
MENGXIA_DB_WRITE_QUEUE
MENGXIA_HASH_CONCURRENCY
MENGXIA_INGEST_SHUTDOWN_TIMEOUT_MS
MENGXIA_LIBRARY_ROOT
MENGXIA_MAX_CLIENT_SESSIONS
MENGXIA_MAX_CONCURRENT_INGESTS
MENGXIA_MAX_DECODE_DEPTH
MENGXIA_MAX_FRAME_BYTES
MENGXIA_MAX_INGEST_BYTES
MENGXIA_MAX_INGEST_OPERATION_TIMEOUT_MS
MENGXIA_MAX_PENDING_HANDSHAKES
MENGXIA_MAX_STAGING_BYTES
MENGXIA_MIN_FREE_BYTES
MENGXIA_MIN_FREE_PERCENT
MENGXIA_STORAGE_IO_CONCURRENCY
MENGXIA_STREAM_BUFFER_BYTES
```

This external file is Library-specific because it explicitly selects one
`MENGXIA_LIBRARY_ROOT`, but it is not a member of the locked Library namespace. Both
daemon and CLI may read it through the same immutable parsed DTO. A CLI/env override
does not make an invalid configured file acceptable: if a file was explicitly
selected, its authority and complete lexical syntax must validate before any layer
is used. Typed semantic parsing applies only to the value selected by precedence;
an invalid selected higher value fails without falling through, while an unselected
lower value remains bounded text and cannot override the selected value.

Daemon file authority/syntax, selected-value range and impossible-combination
failure maps to `STORAGE_CONFIGURATION_ERROR` (or `STORAGE_IO_ERROR` only for a
certain underlying read failure) and exits before endpoint publication. Client-side
selector/syntax/value failure maps to `VALIDATION_ERROR` with exit status 2. Neither
side includes the path, key or rejected value in its message.

Every daemon value in that closed set has a matching `mengxiad serve` flag (including
`--blob-root`, the three DB flags, all eight storage flags and the three new TASK-007
server/session/shutdown flags). The client has `--library-config` plus its existing
endpoint/frame/depth/handshake flags and `--operation-timeout-ms`. Flag names are the
lowercase kebab-case form of their variables. Duplicate or malformed flags fail
before file access; file authority/syntax is validated before any selected value is
resolved.

Existing TASK-005 keys and limits remain exact:

```text
MENGXIA_LIBRARY_ROOT                 required from CLI/environment/Library config
MENGXIA_BLOB_ROOT                    default <Library>/storage
MENGXIA_STORAGE_IO_CONCURRENCY       default 2, range 1..8
MENGXIA_HASH_CONCURRENCY             default 2, range 1..8
MENGXIA_MAX_CONCURRENT_INGESTS       default 2, range 1..8
MENGXIA_STREAM_BUFFER_BYTES          default 8388608, range 1 MiB..32 MiB
MENGXIA_MAX_INGEST_BYTES             default/max 1 TiB, tightening-only 1..1 TiB
MENGXIA_MAX_STAGING_BYTES            default/max 2 TiB, tightening-only and >= max ingest
MENGXIA_MIN_FREE_BYTES               default/min 10 GiB, increase-only
MENGXIA_MIN_FREE_PERCENT             default/min 5, range 5..100
```

TASK-007 adds one server operation ceiling, one client-requested tightening value,
one session cap and one graceful-shutdown budget:

```text
MENGXIA_MAX_INGEST_OPERATION_TIMEOUT_MS
  server default/max: 86400000 (24 h)
  accepted: 100..86400000, tightening-only

MENGXIA_CLIENT_OPERATION_TIMEOUT_MS / --operation-timeout-ms
  client default: 3600000 (1 h)
  accepted: 100..server ceiling by the server; client parser accepts 100..86400000

MENGXIA_MAX_CLIENT_SESSIONS
  default: 32
  accepted: 1..256

MENGXIA_INGEST_SHUTDOWN_TIMEOUT_MS
  default: 5000
  accepted: 100..30000
```

The server starts the operation deadline only after the complete request has passed
frame/decode/semantic validation. Handshake retains its independent 100..5000 ms
budget and existing `MENGXIA_MAX_PENDING_HANDSHAKES` admission. After authentication
and negotiation, a single-command connection must atomically obtain a separate
client-session permit; otherwise the server returns bounded `BACKPRESSURE` before
reading or claiming a product request. It then releases the handshake permit. Thus
slow product commands cannot consume the unauthenticated handshake pool, and at
most 32 product sessions are resident by default; storage and store retain their
lower atomic admission/queue limits.

The application service additionally owns two bounded in-process admission layers
after semantic digest construction and before durable claim; the daemon supplies
their validated capacities at composition:

1. `ActiveIngestBindings`, capacity `MENGXIA_MAX_CLIENT_SESSIONS`, atomically maps a
   command ID to the exact operation/request-digest binding under the one already
   authenticated Library owner. An exact active duplicate returns
   `COMMAND_IN_PROGRESS`; a different binding returns non-disclosing `CONFLICT`.
   A different peer UID never reaches this registry. It is only a live-process
   accelerator; TASK-006 remains the durable principal/binding authority.
2. The first/new binding performs a non-blocking acquire of an execution permit
   whose capacity is
   `min(MENGXIA_STORAGE_IO_CONCURRENCY, MENGXIA_HASH_CONCURRENCY,
   MENGXIA_MAX_CONCURRENT_INGESTS)`. If unavailable, its registry entry is removed
   and it returns pre-claim `BACKPRESSURE`, so the same command may retry. On
   acquisition it holds the permit through claim, CAS and disposition/completion.

The registry entry has an RAII owner and is removed on every terminal session path;
panic/join failure still arms the accepted runtime-failure behavior. This prevents
the default 32 sessions from durably terminalizing command IDs merely because the
default storage capacity is 2, while preserving exact-duplicate AC-007 behavior.
Physical TASK-005 admission may still reject after claim because filesystem
capacity can change between preflight and storage admission; that certain no-effect
result is durably terminal for the command. Values at min/max and
min-1/max+1, empty, whitespace, sign, non-Unicode, overflow, higher-layer invalidity
and impossible reserve/staging combinations are tested before endpoint publication.

After ServerHello, the server gives the client one fresh `HandshakeLimits.timeout()`
budget to deliver and decode the sole `CoreRequest`; stalling consumes only a
session permit for at most 5 seconds and returns `DEADLINE_EXCEEDED` without claim.
After app completion it gets one fresh same-sized transport budget to encode, write,
flush and close the terminal `CoreResponse`. The semantic ingest timeout does not
silently lengthen either transport budget. The client bounds its total post-handshake
request/write/read wait by its requested operation timeout and closes on expiry; a
lost terminal response is resolved only by replaying the same command ID.

## 8. Exact ingest state machine

### 8.1 Admission and claim

```text
S0 accept UDS under bounded pending-handshake semaphore
S1 authenticate peer UID before first frame
S2 negotiate ClientHello/SINGLE_COMMAND
S3 atomically transfer to one bounded client-session permit
S4 read/preflight/decode exactly one request frame
S5 validate mode, command ID, timeout and semantic values
S6 open opaque TASK-005 source; validate normalized source selector
S7 compute canonical digest and CommandBinding(asset.ingest.v1)
S8 bind active command and non-blockingly acquire bounded execution permit
S9 durable claim through AssetUnitOfWork
S10 if Claimed: invoke BlobStorage::ingest
S11 if Stored: sample completion time, generate plan/event IDs and synchronously
    complete the TASK-006 transaction
S12 encode/write one result and close
```

No filesystem/database mutation occurs before S9. Opening and reading metadata from
the source at S6 is read-only. The in-process outcomes at S8 are exact active
duplicate/conflict/pre-claim-backpressure results described in §7. The durable
outcomes at S9 are exact:

- `InProgress` -> `COMMAND_IN_PROGRESS`, no CAS call;
- `Replay(CommandResult::ManagedRegistration)` -> identical typed result, no CAS
  call; every other `CommandResult` variant is an invariant/storage failure, sends
  no product response, explicitly fails current-runtime mutation and begins failed
  runtime shutdown;
- `TerminalRejected`/`RecoveryRequired` -> stored safe code, no CAS call;
- binding/principal/operation/digest mismatch -> `CONFLICT` without object-existence
  disclosure;
- `Claimed` -> this session alone owns the external effect and armed guard.

A delivered claim `Err` is known by the TASK-006 port contract to have committed no
new CommandRecord or to have already failed the store gate. The app maps only
`Validation`, `Conflict`, `IdGenerationUnavailable`, `StorageBusy`,
`StorageConfiguration`, `Backpressure` and `ShuttingDown` to the exact no-claim
code/retry rows in §10. The existing SQLite writer deliberately changes its
admission gate to `Failed` when a submitted asset job returns `StorageIo`,
`StorageCorruption` or `Internal`; those three claim errors therefore return
`RuntimeFailed`, send no product response and begin failed runtime shutdown even
though no CommandRecord committed. `NotFound`, `InvalidTransition`,
`RevisionExhausted` and every future unknown claim error are also impossible for
this method and take the same fail-closed path rather than being coerced to a
business error. This preserves the TASK-006 fatal-gate contract and prevents retry
guidance from implying that the current runtime can still accept mutation.

The app checks the same non-blocking control at entry, after source open/digest,
immediately before claim, immediately after a successful claim and before invoking
storage. A stop before S9 has no CommandRecord and returns the typed deadline/
cancellation code; a stop after `Claimed` is persisted through the armed guard as a
certain terminal result with one newly sampled `observed_at`. TASK-005 then owns all
in-stream/pre-promote checkpoints.
There is no race window in which a stop drops an armed guard without disposition.

Because TASK-005 has no separable physical-capacity reservation, CAS backpressure
after S9 is stored
as terminal `BACKPRESSURE` for this command. Retry guidance explicitly says a fresh
admission requires a new command ID. No automatic sleep/retry loop exists.

The timing/identity order is also fixed. The app samples `claimed_at` before S9; a
clock failure therefore leaves no claim. It generates no graph/event ID merely for
owning a claim. After `DurableBlob` returns, it samples `completed_at` and only then
generates the five graph/Location IDs and two event IDs. Failure to sample that
post-CAS time leaves the guard armed and closes the runtime. A later ID-generation
failure uses the sampled `completed_at` to persist
`RecoveryRequired(ID_GENERATION_UNAVAILABLE)`; failure to persist that disposition
also leaves the guard armed and closes the runtime. These paths never classify a
durable canonical Blob as a clean pre-effect rejection.

### 8.2 CAS and completion boundary

The app passes the optional expected digest and an `Arc<dyn IngestControl>` into the
blocking storage adapter using one joined `spawn_blocking` task. The task is always
awaited. For `Stopped` and every error known to have no durable effect after claim,
the app samples exactly one `observed_at`, constructs
`ExternalIngestDisposition::TerminalRejected`, awaits `guard.finish`, and only then
returns the error. For a post-CAS error that can be represented as recovery, it
likewise samples exactly one `observed_at` and durably finishes
`RecoveryRequired`. If timestamp sampling, disposition construction or disposition
persistence fails, or cleanup/effect certainty is unavailable, the guard remains
armed and closes current-runtime mutation so reopening classifies the claim as
recovery-required. Panic/join failure and unknown non-exhaustive variants take this
fail-closed path. That path sends no `CoreResponse`; it closes the session and begins
failed runtime shutdown, so the observing client reports uncertain transport and
retries the same command only after restart. No post-claim product response is sent
before its terminal/recovery disposition commit has returned successfully.

The production mapping is exhaustive and tested against the accepted TASK-005 and
TASK-006 enums. `Stopped`, `Validation`, `SourceModified`, certain-no-effect `Io`,
`Corruption`, `Configuration`, `Backpressure` and `EntropyUnavailable` use
`TerminalRejected` with their existing safe code. `RecoveryRequired` and
`StagingNamespaceUnavailable` use
`RecoveryRequired(STORAGE_CONFIGURATION_ERROR)`, matching the accepted TASK-006
mapping for an exhausted/colliding staging namespace. `CleanupFailed`,
`ShuttingDown`, `Internal`, the currently unproduced but unrepresentable `Conflict`,
and every future unknown variant leave the guard armed and close current-runtime
mutation. This mapping adds no new disposition code and does not weaken a TASK-005
same-runtime-forbidden class.

When `Stored(DurableBlob)` returns, cancellation/deadline no longer overrides the
physical fact. The app constructs the exact one-member `ManagedRegistrationPlan`
and must call `ExternalClaimGuard::complete` even if the client disconnected or its
deadline expired. The returned `MutationOutcome` is handled exhaustively:

- `Applied(CommandResult::ManagedRegistration)` and
  `Replay(CommandResult::ManagedRegistration)` return those exact IDs;
- `RecoveryRequired { safe_error_code }` returns that stored recovery response with
  `OPERATOR_OR_RUNTIME_ACTION`;
- `TerminalRejected { .. }` or an applied/replayed non-`ManagedRegistration` result
  violates the accepted TASK-006 method/result matrix. Because the current guard
  disarms on every successful `MutationOutcome`, the app explicitly invokes
  `fail_current_runtime_for_unresolved_external_ingest`, begins failed runtime
  shutdown and sends no product response;
- `Err`, panic or join failure leaves the guard armed, begins failed runtime
  shutdown and sends no product response.

The current guard deliberately disarms on every successful `MutationOutcome`, not
only on applied success. `RecoveryRequired` is the sole valid non-success completion
outcome; impossible successful variants therefore require the explicit fail call
above rather than relying on guard drop. Conversely, no `Err` may be translated to
a clean rejection after CAS. This preserves “orphan allowed, broken Asset
forbidden”.

### 8.3 Disconnect, cancellation and shutdown

After the request frame, the server keeps the read half solely as a disconnect/
protocol-violation watcher. The client keeps its write half open until the terminal
response. EOF, extra bytes, daemon shutdown or elapsed operation deadline set the
cooperative control atomically; they do not abort/drop the joined storage task.
The watcher is a read future owned by the same joined session task, not detached:
the session selects it against the blocking-task join handle. If the app finishes
first, dropping only that pending read future is safe and the response is written
without waiting for client write-half EOF. If EOF/extra input wins, the session sets
control and still awaits the app handle. A normal client that keeps its write half
open until receiving the response therefore cannot deadlock completion.

The supervisor has distinct pre-product handshake and product-session ownership.
Transition to the joined product-session set is atomic before the handshake permit
is released or app work can be dispatched. Shutdown may abort only a task proven to
still be in pre-product handshake/frame admission; it signals and joins every task
that could own a claim, CAS work or completion. Reusing the current daemon's blanket
`JoinSet::abort_all` after TASK-007 product dispatch is forbidden.

A response write/flush/close failure after an app result or stored disposition is a
per-session transport loss; it never rolls back the durable state and does not by
itself fail global mutation admission. The server closes that session, and the
client reports uncertain transport and replays the same command ID. By contrast,
constructing an impossible response from an invalid typed app outcome is an
invariant and follows `RuntimeFailed`.

- before durable promote: TASK-005 cleans its own staging and returns typed stopped;
- during non-cancellable publish: server waits for the bounded storage call;
- after `DurableBlob`: registration is mandatory as described above;
- shutdown stops new accepts/claims, signals active pre-promote work, joins every
  session, drops the app service, unwraps/shuts down storage, cleans the runtime
  endpoint, then calls `OpenedLibrary::shutdown` so store workers/SQLite close and
  the durable Library lock is released last;
- after all joined sessions and the composition service owner are dropped,
  `Arc::try_unwrap(LocalBlobStorage)` must succeed. A remaining strong owner is a
  leaked-work invariant: the controller takes the same immediate `process::exit(1)`
  path without unwinding the returned `Arc`, rather than reporting graceful
  shutdown or invoking its blocking `Drop`;
- if `MENGXIA_INGEST_SHUTDOWN_TIMEOUT_MS` is exhausted, the daemon does not call
  storage/store clean shutdown, does not unwind owners whose `Drop` implementations
  synchronously join, and does not claim endpoint/lock cleanup succeeded. The
  top-level shutdown controller immediately calls `std::process::exit(1)` from this
  branch. `process::exit` skips Rust destructors; OS process termination releases
  descriptors/locks and ends remaining threads. Returning from `serve`, dropping
  `LocalBlobStorage`/`OpenedLibraryOwner`, or relying on Tokio
  `shutdown_timeout` is forbidden on this timeout branch because those ordinary
  drops can block past the promised bound. Restart may see only the already
  specified TASK-005 orphan/TASK-006 recovery states. A subprocess watchdog proves
  the configured wall-clock bound and successful restart. A task that outlives
  graceful shutdown is never allowed to keep running in a still-serving daemon or
  another process.

## 9. Authorization, isolation and data exposure

- AuthN: exact TASK-003 UDS peer UID equals durable Library owner UID.
- AuthZ: V1 TASK-007 ordinary owner may invoke copy ingest. No Project exists yet;
  project fields are absent/reserved rather than accepted and ignored.
- Principal: only server-derived `PrincipalContext`; store independently binds its
  durable owner UID. A request cannot supply or override either.
- Tenant model: one opened Library/owner is the complete current isolation domain.
- Secrets: none are accepted. Source bytes/path, endpoint, UID, backend/locator,
  SQLite/errno and arbitrary input never enter errors/logs.
- Observable success returns only typed IDs and Blob digest. A command-binding
  conflict returns no prior result or existence detail.
- Source file is never modified, moved, deleted, chmodded or xattr-mutated.
- CAS root/path/lock/file descriptors never cross storage/platform boundaries.

## 10. Error, retry and response contract

No new `ErrorCode` is required. Every operation failure uses the following exact
core-proto-owned static message registry, empty `safe_details`, and the exact
wire-visible retry action from §5. Server encode and Client validation call the same
total function over this allowlist; an unlisted global `ErrorCode` has no operation
wire representation and must fail the current session/runtime as required by its
origin rather than falling through to a generic message.

| Allowed code | Exact `safe_message` |
|---|---|
| `VALIDATION_ERROR` | `request validation failed` |
| `AUTHENTICATION_ERROR` | `client authentication failed` |
| `CONFLICT` | `operation conflicts with durable state` |
| `SOURCE_MODIFIED_DURING_INGEST` | `source changed during ingest` |
| `STORAGE_IO_ERROR` | `storage operation failed` |
| `STORAGE_CORRUPTION` | `storage integrity verification failed` |
| `STORAGE_BUSY` | `storage is temporarily busy` |
| `STORAGE_CONFIGURATION_ERROR` | `storage configuration is unsupported or unsafe` |
| `IPC_TRANSPORT_ERROR` | `local IPC transport failed` |
| `PROTOCOL_VERSION_UNSUPPORTED` | `protocol version is unsupported` |
| `DEADLINE_EXCEEDED` | `operation deadline exceeded` |
| `OPERATION_CANCELLED` | `operation was cancelled` |
| `BACKPRESSURE` | `operation admission is full` |
| `COMMAND_IN_PROGRESS` | `command is already in progress` |
| `ID_GENERATION_UNAVAILABLE` | `identifier generation is unavailable` |

Protocol 1.0 retains its exact existing messages for the codes it can emit; the
table deliberately uses those same strings for `VALIDATION_ERROR`,
`PROTOCOL_VERSION_UNSUPPORTED` and `ID_GENERATION_UNAVAILABLE`. There is no
`_ => "operation failed"` fallback for a protocol-1.1 operation envelope.
Authentication and version rejection occur before an operation session exists, so
their protocol-1.0-compatible handshake envelopes keep `retry_action` absent; the
CLI maps those two typed handshake failures to the table action locally. No other
operation code may omit the field or enter the wire merely because it exists in the
global non-exhaustive `ErrorCode` enum.

| Condition | Code | Durable state | Retry action |
|---|---|---|---|
| malformed frame/proto/path/value/mode/digest/ID/timeout | `VALIDATION_ERROR` | no claim when found before S9 | `NONE` |
| storage validation discovered only after claim | `VALIDATION_ERROR` | `TerminalRejected(observed_at)` | `FRESH_COMMAND` |
| peer mismatch/unavailable credential | `AUTHENTICATION_ERROR` | no frame disclosure, no claim | `OPERATOR_OR_RUNTIME_ACTION` |
| incompatible protocol before operation request | `PROTOCOL_VERSION_UNSUPPORTED` | no claim | `OPERATOR_OR_RUNTIME_ACTION` |
| active/durable binding or principal mismatch | `CONFLICT` | prior state untouched; no result disclosure | `NONE` |
| active or durable exact claim | `COMMAND_IN_PROGRESS` | no new effect | `SAME_COMMAND` |
| session/registry/execution/store admission full before claim | `BACKPRESSURE` | no record | `SAME_COMMAND` |
| source changed during open before claim | `SOURCE_MODIFIED_DURING_INGEST` | no record | `SOURCE_STABLE_SAME_COMMAND` |
| source changed after claim | `SOURCE_MODIFIED_DURING_INGEST` | `TerminalRejected(observed_at)` | `SOURCE_STABLE_FRESH_COMMAND` |
| physical storage admission full after claim | `BACKPRESSURE` | `TerminalRejected(observed_at)` | `FRESH_COMMAND` |
| deadline/cancel before claim | `DEADLINE_EXCEEDED` / `OPERATION_CANCELLED` | no record | `SAME_COMMAND` |
| clean deadline/cancel after claim and before promote | `DEADLINE_EXCEEDED` / `OPERATION_CANCELLED` | `TerminalRejected(observed_at)` | `FRESH_COMMAND` |
| SQLite busy before a claim commits | `STORAGE_BUSY` | no record | `SAME_COMMAND` |
| source/blob-storage I/O known pre-effect | `STORAGE_IO_ERROR` | no record before claim, otherwise `TerminalRejected(observed_at)` when cleanup is proven | pre-claim `SAME_COMMAND`; terminal `FRESH_COMMAND` |
| store claim returns `StorageIo`, `StorageCorruption` or `Internal` | no product response; client observes `IPC_TRANSPORT_ERROR` | no new claim; store gate is failed | client `SAME_COMMAND` after restart; persistent corruption requires operator action at startup |
| store is shutting down before claim admission | `STORAGE_IO_ERROR` | no record; current runtime unavailable | `OPERATOR_OR_RUNTIME_ACTION` |
| post-CAS clock failure | no product response; client observes `IPC_TRANSPORT_ERROR` | armed guard/runtime failure; recovery on reopen | client `SAME_COMMAND` after restart |
| post-CAS ID failure with stored disposition | `ID_GENERATION_UNAVAILABLE` | `RecoveryRequired(completed_at)` | `OPERATOR_OR_RUNTIME_ACTION` |
| any exact stored `RecoveryRequired` replay | its allowlisted static safe code | existing recovery row unchanged | `OPERATOR_OR_RUNTIME_ACTION` |
| post-CAS ID/disposition persistence failure | no product response; client observes `IPC_TRANSPORT_ERROR` | armed-guard runtime failure; recovery on reopen | client `SAME_COMMAND` after restart |
| any other failure after `DurableBlob` and before proven completion | underlying safe code only when a recovery disposition commits; otherwise no product response | stored recovery or armed-guard runtime failure | stored: `OPERATOR_OR_RUNTIME_ACTION`; uncertain transport: client `SAME_COMMAND` after restart |
| digest/row/schema/locator mismatch | `STORAGE_CORRUPTION` | no automatic mutation/retry | `OPERATOR_OR_RUNTIME_ACTION` |
| unsafe root/orphan/prior-runtime claim or staging namespace unavailable | `STORAGE_CONFIGURATION_ERROR` | no claim at startup, or `RecoveryRequired(observed_at)` after claim | `OPERATOR_OR_RUNTIME_ACTION` |
| storage configuration/corruption/ID/internal failure already proven terminal after claim | corresponding static safe code | `TerminalRejected(observed_at)` | `OPERATOR_OR_RUNTIME_ACTION` |
| pre-claim application ID/clock unavailable | `ID_GENERATION_UNAVAILABLE` | no claim | `OPERATOR_OR_RUNTIME_ACTION` |
| transport uncertainty after request send | `IPC_TRANSPORT_ERROR` on observing side | server state unknown | `SAME_COMMAND` |
| panic/join/invariant/unknown variant after claim | no product response; client observes `IPC_TRANSPORT_ERROR` | close current runtime; recovery on reopen | client `SAME_COMMAND` after restart |

`ErrorEnvelope.retryable` is operation-context-specific, not derived from
`ErrorCode` alone, and is mechanically checked against `retry_action`. In
particular a stored terminal `BACKPRESSURE`, deadline or cancellation response uses
a fresh command, while `COMMAND_IN_PROGRESS`, pre-claim admission failure and
lost-response uncertainty use the same command. A received post-CAS recovery error
is never advertised as a fresh automatic retry; durable state must be inspected
after restart. A client-local deadline or IPC failure after request transmission is
uncertain even if the server later completes, so it also uses `SAME_COMMAND`.
Replay derives the same action from the typed TASK-006 claim outcome plus stored
safe code (`TerminalRejected` versus `RecoveryRequired`); no new mutable retry field
is added to immutable migration `0001`.

## 11. CLI contract

The exact new grammar is:

```text
mengxia asset ingest-copy SOURCE
  --command-id UUIDV7
  --asset-kind TOKEN
  --content-kind TOKEN
  --representation-purpose TOKEN
  --resource-kind TOKEN
  --logical-name UTF8
  [--expected-sha256 LOWERCASE_HEX_64]
  [--operation-timeout-ms ASCII_U64]
  [--library-config ABSOLUTE_PATH]
  [existing client endpoint/frame/depth/handshake options]
```

`--command-id` is mandatory so scripts can safely retry a lost response. A separate
future convenience flag may generate and print one only after its retry UX is
reviewed; TASK-007 does not hide an ephemeral idempotency key. `SOURCE` is one raw
`OsString`; option names and semantic text must be Unicode. Duplicate, combined
`--x=y`, missing or unknown options fail locally before connect.

Success is exactly one LF-terminated stdout line in this field order:

```text
MENGXIA_ASSET_INGEST_OK operation=asset.ingest.v1 asset_id=<uuidv7> asset_revision_id=<uuidv7> representation_id=<uuidv7> resource_id=<uuidv7> location_id=<uuidv7> blob_sha256=<64-lowercase-hex>
```

Failure is exactly
`MENGXIA_ERROR code=<STABLE_CODE> retry=<NONE|SAME_COMMAND|FRESH_COMMAND|SOURCE_STABLE_SAME_COMMAND|SOURCE_STABLE_FRESH_COMMAND|OPERATOR_OR_RUNTIME_ACTION>`
plus LF on stderr and no stdout. The daemon-provided operation retry action is
printed unchanged; client-local validation uses `NONE`, and post-request transport
uncertainty uses `SAME_COMMAND`. Before the operation request is transmitted, a
connect/IPC failure also uses `SAME_COMMAND`, while authentication or protocol
failure uses `OPERATOR_OR_RUNTIME_ACTION`. This exact line applies to the new asset
command; the existing `mengxia handshake` subcommand remains byte-for-byte
unchanged. Exit status 2
means local argument/config validation; status 1 means authenticated operation or
transport failure; success is 0. No path, UID, endpoint, backend, locator or raw
input is printed. Existing `mengxia handshake` output and behavior remain exact.

## 12. Root identity behavior retained by TASK-007

At composition, `OpenedLibrary` mints authority for the resolved Blob-root request
and `LocalBlobStorage::start` returns the current backend ID. Before endpoint
publication, the daemon invokes the exact read-only store seam from §4. TASK-007 may
ingest only when it proves there is no incompatible prior managed backend binding.

Tests cover:

1. default root first use;
2. configured external root first use;
3. rename of the same APFS directory inode: backend ID/locator and replay unchanged;
4. real APFS copy/recreation and the retained checked-device seam for cross-volume
   identity: backend ID differs, no Location is rewritten, old managed custody is
   not claimed under the new root, ingest remains disabled pending a future
   explicitly Admin-gated reconciliation;
5. missing/corrupt old objects never trigger implicit rebind.

The backend preflight also seeds outside-local-family Location rows and proves this
local-only check does not block startup merely because they exist. It separately
proves that malformed
`mengxia.local-cas.v1/` values, multiple distinct local instance IDs and a local ID
mismatch all fail closed without revealing which row caused the failure.

The implementation must not expose a temporary “accept both backend IDs” alias.
TASK-005 staging orphans are different from a backend mismatch: the accepted
storage contract permits new atomic admission against safely remaining capacity.
The daemon retains that behavior and reports every nonzero startup summary once as
the exact redacted diagnostic
`MENGXIA_STORAGE_STATUS state=ORPHAN_RECONCILIATION_REQUIRED orphan_count=<u16> orphan_bytes=<u64>`.
It never prints a name/path/digest and never deletes the orphan. A request that the
existing admission logic cannot fit returns its existing recovery/backpressure
class. TASK-008 may verify/report the affected custody only. No current task owns a
rebind mutation; assigning one requires resolution of OQ-010 and a separate
Admin-gated start contract.

TASK-007 itself exits before endpoint publication on a local-backend mismatch.
After its own gate is accepted, TASK-008 may replace only that startup behavior with
a typed degraded verification/report mode so the user can inspect affected custody;
copy ingest and all binding mutation remain disabled in that mode. This explicit
extension point prevents TASK-007 from pre-implementing recovery while avoiding a
future requirement to weaken the fail-closed check.

## 13. Crash, fault and concurrency registry

The following named boundaries are fixed before activation. Each SIGKILL case runs
in a child process, reopens through production startup and asserts exact filesystem,
command, graph/event and response-replay state. Fault seams are used for errors that
SIGKILL cannot deterministically place.

| ID | Boundary after returned action | Allowed restart result |
|---|---|---|
| `KILL-007-001` | authenticated request validated, before source open | no command/blob/graph |
| `KILL-007-002` | source opened, before claim submit | no command/blob/graph |
| `KILL-007-003` | claim transaction committed | prior-runtime `RECOVERY_REQUIRED`; no graph |
| `KILL-007-004` | staging name created | recovery claim + TASK-005 staging orphan; no graph |
| `KILL-007-005` | first chunk durable | same as 004 |
| `KILL-007-006` | all bytes written/hashed, before staging sync | same as 004 |
| `KILL-007-007` | staging file full-sync returned | same as 004 |
| `KILL-007-008` | canonical no-replace publish returned | recovery claim; canonical orphan allowed |
| `KILL-007-009` | shard sync returned | recovery claim; durable canonical orphan allowed |
| `KILL-007-010` | `DurableBlob` returned, before completion submit | recovery claim; no graph |
| `KILL-007-011` | completion transaction begun/mid-statement | rollback or fully completed, never partial graph/event |
| `KILL-007-012` | completion COMMIT returned, before response write | exact same-command replay of original result |
| `KILL-007-013` | response frame written, before close | completed replay; no duplicate effect |
| `KILL-007-014` | terminal/recovery disposition transaction begun/mid-statement | prior claim becomes recovery-required or exact stored disposition; no CAS/graph beyond the already proven prefix |
| `KILL-007-015` | disposition COMMIT returned, before error response write | exact same-command replay of the stored safe error; no CAS/graph |
| `KILL-007-016` | terminal error response frame written, before close | exact stored-error replay; no duplicate effect |

Fault coverage includes every TASK-005 staging/promote/cleanup group and every
TASK-006 claim/completion statement group through retained gates, plus TASK-007
frame read/write, decode, clock/ID generation, blocking-task spawn/join, disconnect,
deadline and shutdown transitions. New wrappers do not duplicate all lower-level
fault points; the aggregate proves they remain reachable end to end.

Concurrency tests use barriers, not timing guesses:

- two authenticated sessions, same exact command/request: one claim/CAS/graph/event;
  peer gets in-progress or exact replay. The fixture must pass both sessions clones
  of the same production-shaped service `Arc`; a test that constructs two services
  is not evidence for AC-007;
- exact duplicate while the first command is only in `ActiveIngestBindings`, and
  exact duplicate while execution permits are saturated: bounded in-progress/same-
  command behavior, never a second claim or false conflict;
- same command with each semantic field changed separately, source selector changed
  and different principal fixture: conflict, no disclosure/effect;
- two commands/same bytes: distinct Assets and shared Blob;
- max sessions, registry capacity, derived execution permits, physical storage
  admission, store queue and cap+1 overload: bounded backpressure; pre-claim
  saturation does not create/terminalize a CommandRecord;
- disconnect/deadline/shutdown races immediately before and after promote/commit;
- source rename/write/truncate/append/metadata swap across every read boundary.

Stress repeats race cases at least 100 iterations in formal mode. Deterministic
barriers provide correctness evidence; repetition is supplementary.

## 14. Stable TEST registry

| Test ID | Required evidence |
|---|---|
| `TEST-PROTO-007` | exact descriptor/provenance/tags/reservations/per-root depth/preflight; legacy depth 3 remains accepted; retry-action presence/validity and handshake compatibility |
| `TEST-CLI-007` | exact grammar, raw source bytes, exit/output/redaction and thin-CLI architecture |
| `TEST-CONFIG-007` | four-layer storage/deadline resolution, byte-preserving parse followed by retained Unicode-only roots, and all finite boundaries before mutation |
| `TEST-AUTH-007` | real peer UID, no actor/project/Admin field, conflict non-disclosure |
| `TEST-DIGEST-007` | selector and full canonical-request golden vectors, fixed metadata-absence marker and field sensitivity |
| `TEST-INGEST-007` | complete normal copy flow, mandatory absent Blob media metadata, expected digest and zero/large bounded stream |
| `TEST-SOURCE-007` | non-Unicode/invalid/symlink/type/mutation/EOF source matrix; source unchanged |
| `TEST-CUSTODY-007` | durable promote before graph, shared Blob/distinct Asset and opaque Location |
| `TEST-COMMAND-007` | replay/conflict/in-progress/terminal/recovery exact outcome matrix, including fail-closed impossible completion variants |
| `TEST-CONCURRENCY-007` | active-binding duplicates, pre-claim/physical saturation and exactly-one effect/event |
| `TEST-CANCEL-007` | deadline/disconnect/shutdown before/after promote; no detached work |
| `TEST-RECOVERY-007` | all sixteen KILL boundaries plus retained fault groups |
| `TEST-ROOT-007` | first root, same-inode rename, changed-instance fail-closed/no rewrite, non-local row coexistence and indexed bounded query plans |
| `TEST-ERROR-007` | complete code/durable-state/retry-action/envelope/static-message/redaction matrix |
| `TEST-LIFECYCLE-007` | non-deadlocking read watcher, pre-product-only abort, joined graceful ordering, leaked-Arc/fatal-timeout subprocess exits without blocking Drop unwind |
| `TEST-ARCH-007` | dependency graph, proto/domain/SQLite/path boundaries and exact file scope |
| `TEST-SUPPLY-007` | locked offline build, descriptor attestation, advisories/licenses/sources |
| `TEST-DOC-007` | proposal/ADR/spec/plan/AC/TEST/lifecycle and Admin-gated future rebind ownership alignment |
| `TEST-ENDTOEND-007` | real CLI→daemon→CAS→SQLite→response/replay on APFS |

Every ID must map to a non-empty command/function in
`scripts/verify-task-007.sh`. Developer mode may skip only explicitly named real
second-UID, large-file and SIGKILL stress cases and prints `FAST_PASS`; completion
requires every ID to print `PASS` on the exact committed candidate in reviewed
`macos-26` formal CI. Retained TASK-006 formal coverage is aggregated once; the
TASK-003 real-second-UID job remains separate.

## 15. Acceptance criteria and evidence mapping

The canonical ACs are interpreted exactly as follows:

```text
AC-001 copy-only E2E persists one complete managed graph/event set only after a
       verified durable Blob and returns five typed IDs plus the Blob digest.
AC-002 two commands with identical bytes create distinct Assets and one Blob digest.
AC-003 source mutation returns SOURCE_MODIFIED_DURING_INGEST and no canonical graph.
AC-004 crash after promote/before commit leaves no broken Asset; orphan plus durable
       recovery-required claim is reported, never guessed complete or deleted.
AC-005 identical completed command/request replays the exact original result only.
AC-006 same command with any different semantic request returns CONFLICT.
AC-007 concurrent exact duplicates produce one effect/event set; peer sees in-progress
       or exact replay.
AC-008 command bound to another principal/operation conflicts without disclosing
       prior result or object existence.
AC-009 unspecified/adopt/reference/unknown mode is rejected before source/CAS mutation.
```

| AC | Mandatory evidence |
|---|---|
| AC-001 | PROTO, INGEST, CUSTODY, E2E |
| AC-002 | INGEST, CUSTODY, CONCURRENCY |
| AC-003 | SOURCE, COMMAND, RECOVERY |
| AC-004 | RECOVERY, COMMAND, CUSTODY, E2E |
| AC-005 | DIGEST, COMMAND, E2E |
| AC-006 | DIGEST, COMMAND, AUTH |
| AC-007 | COMMAND, CONCURRENCY, RECOVERY |
| AC-008 | AUTH, COMMAND, ERROR |
| AC-009 | PROTO, CLI, SOURCE, COMMAND |

Security completion must explicitly report `SEC-005`, `SEC-013`, `SEC-017`,
`SEC-020` and `SEC-021`; reliability completion reports `REL-001`, `REL-004`,
`REL-005`, `REL-006`; data completion reports `DATA-002`, `DATA-003`, `DATA-004`,
`DATA-009`, `DATA-013`. No item may be inferred from code appearance alone.

## 16. Canonical traceability correction

The TASK-007 canonical row should consume:

```text
FEATURE: FUNC-002
REQUIREMENTS:
  REQ-001, REQ-002, REQ-008, REQ-010, REQ-011, REQ-013,
  DATA-002, DATA-003, DATA-004, DATA-009, DATA-013,
  API-001, API-002, API-003, API-008, API-010,
  SEC-005, SEC-013, SEC-017, SEC-020, SEC-021,
  REL-001, REL-004, REL-005, REL-006, PERF-001,
  CFG-001, CFG-003
DECISIONS:
  BASE-007, BASE-009, BASE-011, BASE-013, BASE-014, BASE-015, BASE-016,
  BASE-017, BASE-018,
  DEC-003, DEC-006, DEC-007, DEC-008, DEC-016, DEC-017, DEC-018,
  DEC-019, DEC-020, DEC-021, DEC-022,
  ADR-0002, ADR-0004, ADR-0005, ADR-0007, ADR-0008, ADR-0009
PREREQUISITES: TASK-003 DONE; TASK-005 DONE; TASK-006 DONE
```

Project policy is not traced because Project does not exist yet. `API-009` is not
reopened: TASK-003 already established that Admin is disabled and TASK-007 adds no
Admin request. `SEC-014` is not claimed because classification/rights fields do not
exist in this slice.

## 17. Implementation order after acceptance

```text
STEP-1  Synchronize canonical docs/ADR correction and insert exact TASK-007 start record.
STEP-2  Extend committed proto/descriptor/provenance and retain terminal handshake tests.
STEP-3  Add typed app request/result/retry, canonical digest, bounded active/execution
        admission and control/deadline seams with fakes.
STEP-4  Compose all existing Blob configuration and start LocalBlobStorage before endpoint.
STEP-5  Implement daemon single-command session and exact claim→CAS→completion state machine.
STEP-6  Implement thin CLI grammar, raw-byte source field and stable output/error mapping.
STEP-7  Add E2E, duplicate/saturation, source race, root identity, disposition crash,
        fatal-timeout and recovery fixtures.
STEP-8  Run developer gate, complete diff/security/AC review, then formal committed CI.
STEP-9  Mark DONE and revoke authority only after reviewed formal evidence.
```

No step automatically enters TASK-008.

## 18. Candidate start record

This block may be copied to the Plan only after independent review, acceptance of
the §2.5 ownership correction, canonical synchronization and explicit user
authorization:

```text
TASK007_CANONICAL_GATE: ACCEPTED
TASK007_LIFECYCLE: IN_PROGRESS
TASK007_IMPLEMENTATION_AUTHORITY: TASK_007_ONLY

SCOPE: TASK-007 ONLY — authenticated single-command copy ingest from CLI through
       durable CAS and atomic Asset registration; changed storage instances fail
       closed and are not rebound.
FEATURE: FUNC-002
REQUIREMENTS:
  REQ-001, REQ-002, REQ-008, REQ-010, REQ-011, REQ-013,
  DATA-002, DATA-003, DATA-004, DATA-009, DATA-013,
  API-001, API-002, API-003, API-008, API-010,
  SEC-005, SEC-013, SEC-017, SEC-020, SEC-021,
  REL-001, REL-004, REL-005, REL-006, PERF-001, CFG-001, CFG-003
PREREQUISITES: TASK-003 DONE; TASK-005 DONE; TASK-006 DONE
DECISIONS:
  BASE-007, BASE-009, BASE-011, BASE-013, BASE-014, BASE-015, BASE-016,
  BASE-017, BASE-018,
  DEC-003, DEC-006, DEC-007, DEC-008, DEC-016, DEC-017, DEC-018,
  DEC-019, DEC-020, DEC-021, DEC-022,
  ADR-0002, ADR-0004, ADR-0005, ADR-0007, ADR-0008, ADR-0009
ACCEPTANCE: AC-001, AC-002, AC-003, AC-004, AC-005, AC-006, AC-007, AC-008, AC-009
TESTS:
  TEST-PROTO-007, TEST-CLI-007, TEST-CONFIG-007, TEST-AUTH-007,
  TEST-DIGEST-007, TEST-INGEST-007, TEST-SOURCE-007, TEST-CUSTODY-007,
  TEST-COMMAND-007, TEST-CONCURRENCY-007, TEST-CANCEL-007,
  TEST-RECOVERY-007, TEST-ROOT-007, TEST-ERROR-007, TEST-LIFECYCLE-007,
  TEST-ARCH-007, TEST-SUPPLY-007, TEST-DOC-007, TEST-ENDTOEND-007
DEVELOPER_GATE: scripts/verify-task-007.sh developer
FORMAL_COMPLETION_GATE: scripts/verify-task-007.sh formal
AUTHORIZED_FILES: proposal §3 exact list and restrictions
FORBIDDEN: proposal §3.1; root rebind and TASK-008+ remain unauthorized
```

## 19. Review checklist and fastest path to implementation readiness

The reviewer must answer all of the following before acceptance:

- Does protocol intent preserve exact TASK-003 handshake-only compatibility?
- Is every API-010 dimension explicit for `asset.ingest.v1`?
- Can any request field forge principal, IDs, CAS path or custody?
- Is the canonical digest byte-for-byte deterministic and complete?
- Does caller input remain unable to create the existing shared-Blob media mismatch?
- Can exact duplicates bypass saturation without consuming a second claim while new
  commands receive pre-claim backpressure?
- Does every post-claim outcome become completed, terminal or recovery-required?
- Is retry identity carried end to end rather than inferred from an error code?
- Can cancellation detach work or suppress post-promote registration?
- Does the fatal shutdown branch avoid every blocking owner `Drop` path?
- Do all crash points forbid a broken canonical Asset?
- Are queues, sessions, buffers, bytes and time finite with executable boundaries?
- Is cross-instance root rebinding unambiguously outside this slice and fail closed?
- Do the exact scope, tests and retained gates prevent TASK-008+ implementation?

Fastest safe path:

```text
1. Review and accept/reject the §2.5 root-rebind ownership correction.
2. Resolve any review findings in this proposal only.
3. Synchronize Specification/Plan/Review/Intake/Decisions/ADR and add TEST-DOC-007.
4. Run document, naming, formatting and retained TASK-006 developer baselines.
5. Obtain explicit TASK-007-only authorization; only then modify production code.
```
