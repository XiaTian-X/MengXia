---
title: "TASK-011 Plugin private protocol and hostile fixture start-gate proposal"
project: "梦夏 / MengXia"
document_role: "Accepted TASK-011 implementation supplement"
status: "ACCEPTED_IN_PROGRESS_INCORPORATED_BY_CANONICAL_SPECIFICATION_1_1_52"
version: "0.1.2"
date: "2026-09-13"
canonical_specification_reviewed: "IMPLEMENTATION_SPEC.md v1.1.51"
repository_head_reviewed: "1260bf99dbc9e9bd0af1715510d12c27d2f8ffd7"
---

# TASK-011 Gate Proposal

## 0. Gate verdict

TASK-011 is accepted for the exact private-protocol-only implementation defined
here. TASK-003 and TASK-010 are complete; the user accepted the corrected v0.1.2
caps and boundaries after independent re-review. Canonical Specification v1.1.52,
ADR-0017 and the Plan start record activate only §10's file scope.

```text
TASK011_CANONICAL_GATE: ACCEPTED
TASK011_LIFECYCLE: IN_PROGRESS
TASK011_IMPLEMENTATION_AUTHORITY: TASK_011_PRIVATE_PROTOCOL_ONLY
TASK011_PROPOSAL_VERSION: 0.1.2
TASK011_REPOSITORY_HEAD_REVIEWED: 1260bf99dbc9e9bd0af1715510d12c27d2f8ffd7
TASK011_ENVIRONMENT_PREFLIGHT: PASS_DEVELOPER_NON_ATTESTED
TASK011_DEPENDENCIES: TASK003_DONE,TASK010_FOUNDATION_DONE
TASK011_OQ006_PLUGIN_CAPS: ACCEPTED_TASK011_SUBSCOPE_ADR_0017
TASK011_INDEPENDENT_REVIEW: PASS_2026_09_13
TASK011_REMAINING_GATES: NONE_BEFORE_STEP_1
```

No installation, activation, production process launch, sandbox, Broker, Core,
Admin, persistence, migration, CLI, daemon or TASK-012+ behavior is authorized.

## 1. Repository, environment and baseline

The pre-start inspection established:

- `main` equals `origin/main` at
  `1260bf99dbc9e9bd0af1715510d12c27d2f8ffd7`; the worktree was clean before this
  proposal was added.
- host: arm64 Darwin 25.6.0, macOS 26.6.2 build 25G83;
- selected Xcode 26.6 build 17F113, macOS SDK 26.5;
- Rust/Cargo 1.98.0 for `aarch64-apple-darwin`, LLVM 22.1.8;
- cargo-deny 0.20.2;
- local toolchain fingerprint
  `3cbf8f459741f6ad42d7f662f383eac0bb8338165d777b67d067dd0636f1b40f`
  is developer-compatible but does not claim the hosted formal attestation;
- `scripts/verify-repository.sh docs` passed all document traceability, naming,
  CI-orchestration and CI-evidence tests;
- `scripts/verify-ci-fast.sh` passed as non-formal local evidence;
- `protoc` is intentionally not required on ambient `PATH`. Builds consume a
  checked-in descriptor; regeneration uses the pinned official protoc 35.1 arm64
  macOS archive and recorded checksum through `scripts/verify-proto-artifacts.sh`.

No environment defect blocks implementation. Formal evidence
still belongs to the existing reviewed-PR/macOS CI process after implementation.

## 2. Pre-start gap classification and accepted disposition

| Item | Classification | Scope | Required resolution |
|---|---|---|---|
| Empty `mengxia-plugin-proto` and `mengxia-plugin-host` skeletons | `EXPECTED_GAP` | TASK-011 | implement under the active exact start authority |
| Former `MENGXIA_PLUGIN_LOG_BYTES=TBD` and missing Plugin frame/session/queue caps | `SPEC_STALE / RESOLVED` | TASK-011 global gate | exact finite values accepted under ADR-0017 |
| Formerly missing TASK-011-specific AC or stable TEST registry | `SPEC_STALE / RESOLVED` | completion evidence | canonical §19.12/§20.0.10 now own definitions |
| `mengxia-plugin-host` currently depends on app/ports despite being empty | `REPO_STALE` | TASK-011 architecture | replace with the exact private-protocol edges in STEP-1 |
| Independent review and start record | `EXPECTED_GATE / RESOLVED` | TASK-011 | synchronized by this accepted gate |
| TASK-012 sandbox/backend/managed launch design remains open | `DEFERRED` | TASK-012 | must not block the protocol-only TASK-011 slice |
| Admin/OQ-010, installation, grants and activation remain open | `DEFERRED` | TASK-013 | explicitly outside TASK-011 |
| TASK-010 checks the entire current lock against its historical hash | `REPO_STALE / CONFLICT` | prerequisite test compatibility | retain historical lock evidence and check the exact authorized graph delta under §6.1; do not remove supply obligations |
| CI aggregate knows only the retained baseline and MAINT-003 IDs; draft v0.1.0 allowed display-only changes | `CONFLICT` | evidence integration | append TASK-011 evidence under §9.1 without changing the historical baseline or CI topology |
| Canonical document version data was absent from the candidate file scope | `SPEC_STALE` | canonical synchronization | include task-lifecycle-records.toml; keep version checks executable |
| v0.1.0 omitted pre-write deadlines, drop ownership and legal failure outcomes | `CONFLICT` | REL-006 and protocol interoperability | use §5.3–§5.6; no production process authority is added |

## 3. Exact task ownership

TASK-011 owns only:

1. the `mengxia.plugin.v1` private control protocol v1.0;
2. descriptor-first generation and closed wire validation for that protocol;
3. bounded framed read/write and bounded in-memory queues over caller-supplied
   private streams;
4. a session state machine with finite handshake, request and shutdown budgets;
5. bounded, non-authoritative stderr collection and a typed termination request;
6. a test-only hostile Plugin executable and conformance harness;
7. architecture, supply, protocol, overload and lifecycle evidence.

TASK-011 does not own:

- package installation, approval, grant, revocation or activation;
- executable acquisition, durable custody, pathname resolution or launch;
- `std::process::Command` or equivalent in production crates;
- process kill, sandbox setup, resource-limit syscalls or process-tree custody;
- Client/Admin endpoints, Core request/response types, DB/CAS handles or paths;
- Asset, Network, Credential or other Broker operations;
- daemon/CLI composition, persistence, migration, audit or public product API;
- terminal AC-020 through AC-028 security claims.

TASK-012 owns the managed executable-to-launched-image proof, real process
lifecycle, kill/join escalation, resource enforcement and OS sandbox. TASK-013
owns authenticated install/grant/revoke composition. TASK-016 owns the complete
Broker/egress boundary.

The exact TASK-011 requirement contribution is:

| ID | TASK-011 contribution | Remaining owner |
|---|---|---|
| `FUNC-006` | private protocol compatibility foundation only | install/lifecycle composition remains TASK-013 |
| `FUNC-007` | host-side protocol containment before OS sandbox | terminal native containment remains TASK-012/013/016 |
| `API-001` | completes only Plugin transport proto3 sub-scope | aggregate requirement remains release-owned |
| `API-002` | exact `mengxia.plugin.v1` package | Core namespace remains frozen |
| `API-004` | proves supplied protocol exposes no Core Client/Admin/DB handle | later launch/Broker composition must retain it |
| `SEC-005` | separates the private control type graph from Core authority domains | TASK-012/013 retain process/channel binding |
| `SEC-017` | bounded/canonical validation for this wire and stderr boundary | remains active at every later input boundary |
| `SEC-020` | exact task dependency/protoc evidence | continuous supply review remains active |
| `SEC-021` | accepted finite protocol/session/log caps only | sandbox resources and Provider caps remain later gates |
| `REL-001` | bounded queues and backpressure for private streams | later runtime queues retain separate ownership |
| `REL-006` | propagated deadlines and structured cancellation of TASK-011-owned session work | process kill/join remains TASK-012 |
| `CFG-001` | defines and validates the immutable `PluginHostLimits` DTO only | composition-root source precedence remains with the first task that wires the host |
| `CFG-003` | versions the exact TASK-011 cap sub-scope before use | later sandbox/process-tree/Provider/release caps remain independently gated |

## 4. Accepted `OQ-006` Plugin protocol cap decision

The following atomic set is accepted by ADR-0017 and the canonical configuration
table. Every configurable value is parsed once
by the future composition root into immutable `PluginHostLimits`; the host crate
does not read environment variables.

| Typed field / future key | Default and hard ceiling | Accepted configured range | Counting rule |
|---|---:|---:|---|
| `control_frame_bytes` / `MENGXIA_PLUGIN_FRAME_BYTES` | 262144 | 65536..262144, tightening only | Protobuf payload bytes, excluding the 4-byte header |
| `decode_depth` / `MENGXIA_PLUGIN_DECODE_DEPTH` | 16 | 2..16, tightening only | root message has depth 1; each embedded message adds 1; every v1.0 body needs depth 2 |
| `inbound_frames` / `MENGXIA_PLUGIN_INBOUND_QUEUE` | 16 | 1..16, tightening only | complete decoded frames waiting for dispatch |
| `outbound_frames` / `MENGXIA_PLUGIN_OUTBOUND_QUEUE` | 16 | 1..16, tightening only | complete encoded frames waiting for write |
| `active_sessions` / `MENGXIA_PLUGIN_MAX_SESSIONS` | 4 | 1..4, tightening only | held admission permits, including unopened permits; TASK-012 must separately retain process capacity until reap, even after this protocol permit is released |
| `in_flight_requests` | 1 fixed in protocol v1.0 | not configurable | request admitted and lacking its terminal response |
| `stderr_buffer_bytes` | 8192 fixed | not configurable | ADR-versioned scratch bytes used to drain stderr; bytes are counted and then discarded, never retained as a log |
| `stderr_total_bytes` / `MENGXIA_PLUGIN_LOG_BYTES` | 1048576 | 65536..1048576, tightening only | inclusive lifetime log-byte quota for one session; the first byte beyond it requires termination |
| `handshake_timeout_ms` / `MENGXIA_PLUGIN_HANDSHAKE_TIMEOUT_MS` | 5000 | 100..5000, tightening only | from stream ownership at open through HostHello write/flush and validated PluginHello, bounded also by the caller deadline |
| `request_timeout_ms` / `MENGXIA_PLUGIN_REQUEST_TIMEOUT_MS` | 30000 | 100..30000, tightening only | from first polling the request operation through admission/queue/write/flush and validated response, bounded also by the caller deadline |
| `shutdown_timeout_ms` / `MENGXIA_PLUGIN_SHUTDOWN_TIMEOUT_MS` | 2000 | 100..2000, tightening only | from first polling shutdown through outstanding-request settlement, write/flush, validated ack and local cleanup; no wait for peer EOF |

Checked arithmetic must reject impossible aggregate limits before streams are
accepted. At the hard ceilings, queued control payload is bounded by
`4 sessions * 32 queued frames * 262144 bytes = 33554432 bytes`. A conservative
additional allowance per session is four frame-sized working slots (raw read,
decoded dispatch, encoding and current write), plus `8192` stderr scratch bytes:
`4 * ((32 + 4) * 262144 + 8192) = 37781504` payload/scratch bytes in total.
Moving a frame transfers its slot; cloning or retaining another payload must be
accounted for. Producer admission precedes encoding; no unbounded collection of
blocked send futures or pending request payloads may be owned by the host.
Bounded channel/control metadata is counted separately from variable payloads.
Implementations must not eagerly allocate all capacity. The acceptance test uses
instrumented allocation/accounting and requires the derived payload ceiling, not
an assumed `Vec` implementation overhead.

Backpressure is structural: a full bounded channel suspends the corresponding
reader/writer; there is no unbounded side queue, detached retry or frame drop.
Stderr is drained concurrently to prevent a pipe deadlock. Each bounded scratch
chunk is counted with checked arithmetic and immediately discarded; TASK-011 does
not retain, parse, display or forward arbitrary Plugin log content. Exactly the
cumulative cap is permitted. With remaining allowance R, read at most
`min(stderr_buffer_bytes, R + 1)` bytes, using checked arithmetic; the first excess
byte returns `TerminationRequired::ResourceLimit`. Thus an over-quota read retains
no more than the existing scratch bound and counts at most cap+1 bytes. TASK-011
closes its streams and destroys all owned I/O futures under §5.6, but does not
claim that a separately owned OS process has died. TASK-012 consumes that request and performs
bounded kill/tree cleanup before real activation.

These values are safety ceilings, not throughput SLOs. Benchmarking may later
tighten defaults; widening a hard ceiling requires a new ADR and cap-1/cap/cap+1
evidence.

## 5. Private protocol v1.0

### 5.1 Transport and authority

- Exactly one host-to-Plugin control stream, one Plugin-to-host control stream and
  one Plugin-to-host stderr byte stream are supplied already open.
- The control transport uses the existing four-byte unsigned big-endian length
  prefix. Zero and over-limit lengths fail before payload allocation. A framing or
  decode error permanently poisons the session; resynchronization is forbidden.
- Stdout is protocol-only. Ordinary text, multiple encodings, JSON, log lines and
  trailing bytes are `PLUGIN_PROTOCOL_VIOLATION`. Stderr is untrusted diagnostics,
  never protocol and never authority.
- `PluginHostAdmission::try_acquire` returns an opaque single-use permit before any
  stream is moved. Only `SessionPermit::open` consumes the supplied streams and a
  typed host-side `ExpectedPluginSession`. That context is not an authentication
  primitive: it can be constructed by trusted in-process composition from a
  validated `PackageDigest` and an exact non-zero 32-byte challenge, but it can
  never be constructed from Plugin wire data. No constructor accepts a pathname,
  environment, self-reported actor, endpoint or DB/CAS handle.
- The Plugin never receives Core `ClientHello`, `CoreRequest`, `CoreResponse`,
  Admin types, SQLite/CAS paths, bearer data or a generic OS handle over this
  protocol.
- `package_digest` and future `plugin_instance_id` remain host-side expected
  context. A Plugin cannot authenticate itself by echoing either value.
- A host-created 32-byte session challenge only detects crossed/stale control
  streams. It is not a Credential, bearer token, Admin proof or sandbox evidence.
- Only trusted in-process composition supplies transport implementations. Their
  poll and Drop operations must be nonblocking and must close their owned channel
  endpoint on drop. This is not containment of malicious Rust code in the host.
  TASK-012 must satisfy this adapter contract without changing the wire protocol.

### 5.2 Exact candidate schema

Canonical synchronization accepts the following exact schema contract; STEP-2
creates `proto/plugin/v1/control.proto`, its descriptor and provenance. Review may
amend this block before acceptance. Thereafter this version's artifacts remain
frozen; a later owning task may add a separately reviewed protocol version while
retaining historical vectors, rather than treating the current file as immutable
for the lifetime of the project.

```proto
syntax = "proto3";

package mengxia.plugin.v1;

message HostEnvelope {
  uint64 sequence = 1;
  oneof body {
    HostHello hello = 2;
    PingRequest ping = 3;
    ShutdownRequest shutdown = 4;
  }
  reserved 5 to 15;
}

message PluginEnvelope {
  uint64 sequence = 1;
  oneof body {
    PluginHello hello = 2;
    PingResponse ping = 3;
    ShutdownResponse shutdown = 4;
    PluginFailure failure = 5;
  }
  reserved 6 to 15;
}

message HostHello {
  uint32 protocol_major = 1;
  uint32 protocol_minor = 2;
  bytes session_challenge = 3;
  uint32 max_frame_bytes = 4;
  uint32 max_in_flight_requests = 5;
  reserved 6 to 15;
}

message PluginHello {
  uint32 protocol_major = 1;
  uint32 protocol_minor = 2;
  bytes session_challenge = 3;
  reserved 4 to 15;
}

message PingRequest {
  fixed64 nonce = 1;
  reserved 2 to 15;
}

message PingResponse {
  fixed64 nonce = 1;
  reserved 2 to 15;
}

message ShutdownRequest {
  reserved 1 to 15;
}

message ShutdownResponse {
  reserved 1 to 15;
}

message PluginFailure {
  PluginFailureCode code = 1;
  reserved 2 to 15;
}

enum PluginFailureCode {
  PLUGIN_FAILURE_CODE_UNSPECIFIED = 0;
  PLUGIN_FAILURE_CODE_UNSUPPORTED_REQUEST = 1;
  PLUGIN_FAILURE_CODE_INVALID_REQUEST = 2;
  PLUGIN_FAILURE_CODE_INTERNAL = 3;
  reserved 4 to 31;
}
```

This protocol is deliberately capability-free. `Ping` proves request/response
correlation, deadline and queue behavior without inventing TASK-014+ operations.
`Shutdown` is a cooperative message only; it neither launches nor kills a process.
Media and resources are never inline control payloads. Future capability or Broker
messages require their owning task, new tags, an accepted schema update and their
own authorization contract.

### 5.3 State machine and linearization points

```text
ADMITTED
  -> HOST_HELLO_SENT
  -> ACTIVE
  -> SHUTDOWN_SENT
  -> CLOSED

Any non-terminal state
  -> FAILED
  -> CLOSED
```

- Limits are validated when constructing the admission controller. `try_acquire`
  reserves one of the configured permits without waiting or receiving streams;
  failure returns BACKPRESSURE without taking stream ownership. Dropping an
  unopened permit releases it. `open` validates the typed context and caller
  deadline, consumes the streams, starts the handshake clock and returns the
  owning SessionDriver plus a bounded request handle. No I/O is spawned by open;
  a delayed first poll must still honor that original handshake deadline.
- The host sends `HostHello` first with sequence `0`. The only legal response is
  `PluginHello` sequence `0`, exact major `1`, minor `0`, exact 32-byte challenge,
  and no unknown/noncanonical wire data.
- Application sequences start at `1`, increase by exactly one without wrapping,
  and only one request may be in flight. Duplicate, stale, skipped, zero or
  exhausted sequences fail closed.
- A PingResponse must match sequence and nonce; a ShutdownResponse must match
  sequence. The only alternative is a legal PluginFailure under §5.5. Any other
  body, including an unsolicited response, is a protocol violation.
- Control EOF before a validated shutdown ack, including while ACTIVE but idle,
  is transport failure. A validated shutdown ack
  immediately ends the protocol session and closes all three local streams;
  peer EOF or OS-process exit is not additionally required for protocol success.
  Extra bytes after local terminal closure are never parsed. If peer EOF is
  already buffered behind a valid ack, the ack still determines clean closure.
- One absolute deadline is computed for each operation as the earlier of the
  caller's monotonic deadline and start + configured budget. Queue reservation,
  request-slot wait, write, flush, read and validation share it; no stage resets
  the budget. A fresh attempt also remains under the original caller budget.
  Before committing a response, recheck deadline/cancellation. If both cancellation
  and expiry are observed, cancellation wins; otherwise an expired budget beats
  response success. An already committed result cannot be replaced.
- The request-slot reservation bounds admitted application requests to one,
  including a request waiting for write. Busy public request admission returns
  BACKPRESSURE without encoding or queuing another request. Internal inbound/
  outbound channels still suspend their I/O producers when full, under the same
  cancellation/deadline context. Wire sequence is allocated only after admission.
- Dropping/cancelling a request before admission consumes no sequence. After
  admission, cancellation, request-handle abandonment or timeout closes the
  session, including during a partial frame write; no late response is reused.
  Shutdown stops new admissions and uses its own absolute deadline, without
  extending an outstanding request's deadline. It waits for that request only
  within both budgets, then sends the next sequence; expiry closes the streams.
- Terminal success/error and abrupt drop use the ownership rules in §5.6. Any
  failed session requires downstream process cleanup, but never performs it here.

### 5.4 Closed wire validation

`prost` decode alone is not the security validator. Before generated-message
decode, a descriptor-derived bounded scanner must reject:

- unknown or reserved field numbers;
- wrong wire types, field number zero and protobuf groups;
- duplicate singular fields and multiple members of one `oneof`;
- non-minimal varints, truncated values and length arithmetic overflow;
- embedded-message depth greater than the accepted limit;
- enum integers outside the exact closed set;
- empty body, zero/over-limit challenge and any invalid semantic value.

The scanner is generated from the checked-in descriptor at build time, as a
Plugin-specific equivalent of the completed Core descriptor/depth evidence. It
uses checked arithmetic, does not recurse beyond the configured depth and retains
no rejected bytes or strings. Binary protobuf is the only accepted representation;
ProtoJSON/TextFormat are not exposed.

### 5.5 Error mapping and retry contract

No new global `ErrorCode` is required.

| Condition | Stable result | Retry / disclosure |
|---|---|---|
| invalid host limits/context | `VALIDATION_ERROR` | reject in typed construction before stream ownership; static field class only |
| session cap or bounded queue full before admission | `BACKPRESSURE` | caller may create one fresh bounded attempt; no payload detail |
| malformed/unknown/noncanonical/oversized/wrong-state frame or stdout text | `PLUGIN_PROTOCOL_VIOLATION` | no retry in same session; static safe message |
| no common major/minor | `PROTOCOL_VERSION_UNSUPPORTED` | no retry until compatible package; do not echo peer values |
| control stream read/write/flush/early EOF, or stderr read error | `IPC_TRANSPORT_ERROR` | fresh session only under caller budget; no errno/path |
| handshake/request/shutdown budget expires | `DEADLINE_EXCEEDED` | no detached wait; fresh session only under caller budget |
| caller cancellation completes structured local cleanup | `OPERATION_CANCELLED` | no automatic retry |
| legal PluginFailure for Ping | `RequestOutcome::Rejected(PluginRequestFailure)` as defined below | request only fails; no automatic retry or peer diagnostic text |
| legal PluginFailure for Shutdown | same typed rejection plus `TerminationRequired::ShutdownRejected` | close session; no cooperative shutdown retry |
| stderr total cap exceeded | `PLUGIN_PROTOCOL_VIOLATION` plus `TerminationRequired::ResourceLimit` internally | no retry for that package/session; static safe result; no stderr bytes |
| task panic/impossible invariant/sequence exhaustion | `INTERNAL_ERROR` plus termination required | correlation only; no panic/input detail |

Existing framing errors map by source: inbound InvalidLength is protocol
violation; Truncated/Transport is IPC_TRANSPORT_ERROR; AllocationUnavailable is
BACKPRESSURE and closes the session. InvalidLimit after validated construction or
an invalid host-generated outgoing frame is INTERNAL_ERROR, not peer misconduct.
An already expired caller deadline returns DEADLINE_EXCEEDED without starting
I/O. Deadline construction uses checked arithmetic; overflow returns
VALIDATION_ERROR before that operation is admitted or written. Clean stderr EOF
disables that drain branch without failing an otherwise
valid control session or polling EOF repeatedly. These cases are included in
TEST-BOUNDS-011 and TEST-LIFECYCLE-011; no global error-taxonomy change is needed.

PluginFailure is legal only after a Ping or Shutdown was sent, for its exact
active sequence. It is never a Hello response. The host-owned closed enum
PluginRequestFailure maps UNSUPPORTED_REQUEST to UnsupportedRequest,
INVALID_REQUEST to InvalidRequest and INTERNAL to Internal. These are peer
request rejections, not new global ErrorCode variants and not proof of a local
host bug. Public formatting is static and does not echo wire integers.
UNSPECIFIED and unknown values remain protocol violations. A valid Ping rejection
settles the one request, returns to ACTIVE and consumes its sequence; the next
request uses the next sequence. A valid Shutdown rejection settles that request
and closes the session with ShutdownRejected. No automatic resend occurs in
either case. Duplicate rejection/response frames are violations.

Raw stderr, decoded unknown data, challenges, package bytes, peer values and OS
diagnostics are absent from `Display`, `Debug`, Core log fields and metrics. The
test-only harness may compare private fixture bytes in memory but never persists
them as production diagnostics.

### 5.6 Structured lifetime and drop contract

The production SessionDriver directly owns the reader, writer, dispatcher,
stderr-drain futures, queues, streams and permit. It polls these concurrently
inside one future; TASK-011 does not spawn Tokio tasks, blocking workers or OS
threads. The driver must yield between bounded work units and service deadline,
cancellation and stderr even when stdout is continuously ready. Per-poll work is
bounded by one configured-size frame per I/O branch and one stderr scratch chunk.
Keep in-progress read_frame/write_frame futures pinned across unrelated branch
completions; recreating them after a select branch wins would discard partial
header/payload progress. Only terminal cancellation may abandon a partial frame,
and it closes the session rather than attempting stream resynchronization.

On explicit cancellation, timeout, protocol failure or successful shutdown, all
child futures and stream/queue resources are destroyed before the terminal result
is returned; the permit is released last. There is no async cleanup await or
background reaper to detach. Dropping the driver, including before its first poll
or during a partial read/write, performs the same synchronous resource cleanup.
Remaining request handles observe closed channels; a dropped driver cannot
deliver a return value and must never be recorded as successful completion.
Dropping the last request handle cancels an otherwise idle driver when next polled.

An unwind caught at the driver polling boundary is converted to INTERNAL_ERROR
after cleanup, with no panic payload in its result. Use std panic/future primitives
and the existing Tokio features; do not change the process-global panic hook.
In-process abort/OOM/runtime shutdown is not given a recoverability guarantee.

The caller owns polling or spawning the outer driver. A caller that spawns it
must retain its handle, cancel/abort and await it before claiming joined closure;
dropping that JoinHandle is not cancellation. TASK-012 must own that outer handle
together with process custody, and treat missing/abandoned protocol completion as
requiring process cleanup. Even a valid Shutdown ack never proves process exit.
Tests cover drop before first poll, pending I/O, full queue, timeout/select drop,
explicit cancellation and caller-owned spawn/abort/await; every path must close
the endpoints, reject outstanding work and restore the exact admission capacity.

## 6. Crate and dependency architecture

The direct dependency kinds must be exactly as follows (all versions/features
reuse the root workspace declarations):

```text
mengxia-framing normal -> tokio                                      [unchanged]
mengxia-plugin-proto normal -> mengxia-framing, prost, tokio
mengxia-plugin-proto build -> prost, prost-build, prost-types, sha2
mengxia-plugin-host normal -> mengxia-plugin-package, mengxia-plugin-proto, mengxia-types, tokio
mengxia-testkit dev -> mengxia-plugin-host, mengxia-plugin-proto, prost, prost-types, tokio,
                      mengxia-plugin-package, mengxia-plugin-security, sha2
```

`mengxia-plugin-host` removes its placeholder `mengxia-app` and `mengxia-ports`
edges. Root Cargo.toml adds only workspace path declarations for plugin-host,
plugin-package and plugin-proto; crate manifests consume those declarations.
Host and proto have no additional dev-dependencies. The fixture binary uses only
std; the testkit integration harness uses the listed dev-dependencies.

Runtime reachable workspace edges must exclude Core proto, daemon/CLI, app, ports,
store, storage and platform filesystem/sandbox; runtime APIs must not spawn
processes/threads, execute shell commands, open paths or network endpoints. Tokio's
already enabled net feature is not authority to call network APIs in these crates.
Neither host nor proto has a direct serde/HTTP/rusqlite dependency. Host does reach
serde/serde_json through the unchanged pure package validator; this is intentional
and does not permit new remote/file resolution features. Build-only prost tooling
may use its existing filesystem/process machinery to generate from the committed
descriptor; it is not linked as production launch authority. Check normal, build
and dev edges separately. Package/security gain no reverse dependency or feature.

No new registry dependency or version is proposed. Existing exact `prost 0.14.4`,
`prost-build 0.14.4`, `prost-types 0.14.4`, `tokio 1.53.1` and the pinned protoc
35.1 artifact are reused. The hostile executable is a testkit-only Rust binary
using `std`; production code never spawns it. If a real implementation proves that
an additional dependency or Tokio feature is necessary, implementation stops and
returns to supply review rather than changing Cargo opportunistically.

### 6.1 Existing-task supply compatibility and evolution

The reviewed baseline Cargo.lock hash is
`302df8141acee77aa58ecb796a53ecbb4faf9f6cd55dc384667bb08e725c0b2e`.
Adding the above edges necessarily changes lock blocks for plugin-host,
plugin-proto and testkit; it does not authorize any third-party package, source,
checksum, version, feature, vendored patch or deny-policy change.

Start from the existing lock and let `cargo tree --offline -p mengxia-plugin-host`
resolve the manifest-edge changes. Do not delete/recreate the lock or run a broad
update: `cargo generate-lockfile --offline` can upgrade cached transitive packages
even without network access. Review the exact delta, then require locked/offline
metadata/build/tests. Only those three workspace blocks may change in STEP-1.

Replace the two whole-current-lock historical assertions in
scripts/verify-task-010.sh and tests/task_010_foundation.rs together. Preserve an
exact historical lock fixture under tests/fixtures/task_011/Cargo.task-010.lock,
validated against the hash above. A shared tests/support/plugin_supply.rs helper
must retain TASK-010's package/security direct-edge, resolved transitive closure,
source/version/checksum and forbidden-feature assertions on the CURRENT graph;
checking only the historical fixture is insufficient. Keep the existing manifest,
schema, digest and offline-validator product assertions unchanged. The shell
mapping must still execute the complete TASK-010 supply test plus ci_supply.

TASK-011 adds final normal/build/dev graph and no-new-third-party evidence, plus
negative mutations for unexpected local edges, added packages, changed source/
checksum/version, enabled remote/file features and altered historical fixtures.
Do not replace the old hash with a new forever-fixed hash of the whole current
lock. Subsequent reviewed tasks may add unrelated workspace edges without editing
TASK-010's historical evidence or product tests; changes to a protected component
closure or third-party inventory still require their own accepted supply review.
No comparison may silently skip build dependencies or the vendored SQLite patch.

An isolated baseline copy with exactly the manifests above resolved offline to
only the three expected workspace-block changes; all other lock blocks remained
byte-identical. This is dependency preflight, not implementation or CI acceptance.

## 7. Test-only hostile Plugin contract

`crates/mengxia-testkit/src/bin/task_011_hostile_plugin.rs` is not a product Plugin
or supported activation path. It accepts one exact test-only action from argv and
uses only inherited stdin/stdout/stderr. Actions are closed and include:

```text
valid_ping
core_protocol_frame
zero_frame
oversized_frame
truncated_header
truncated_payload
unknown_field
duplicate_field
nonminimal_varint
wrong_wire_type
wrong_challenge
wrong_version
wrong_sequence
stdout_text
stdout_flood
stderr_at_cap
stderr_cap_plus_one
stderr_flood
close_before_hello
close_during_response
hang_before_hello
hang_during_response
panic_after_hello
ignore_shutdown
ping_failure_unsupported
ping_failure_invalid
ping_failure_internal
shutdown_failure_unsupported
shutdown_failure_invalid
shutdown_failure_internal
failure_during_hello
failure_unspecified
failure_unknown
duplicate_failure
```

The harness owns spawning, deadline, kill and reap for this fixture. On the current
Unix target it creates three std UnixStream::pair channels, converts each child
endpoint through OwnedFd into Stdio, and marks only the parent endpoints
nonblocking before tokio::net::UnixStream::from_std. The child still uses inherited
stdin/stdout/stderr. This provides real OS streams without Tokio's process feature,
raw-FD unsafe code, blocking pipe-reader tasks or an extra dependency. Parent-side
unused directions are not exposed as protocol capabilities. Test pipes/sockets
are transport evidence only, not production launch or sandbox authority.

Each action has a fixed 10-second harness exercise timeout using tightened
2-second protocol test budgets; cleanup has a separate fixed 5-second timeout.
All actions execute sequentially inside one owning integration test, with exactly
one live child permitted at a time. A second spawn attempt while that permit is
held is a harness failure; dropping the permit before observed reap is also a
failure. Cargo test-process parallelism therefore cannot multiply this fixture's
child count.
Stdout and stderr are driven concurrently. On success, failure or caught test
unwind, close the driver endpoints, kill a still-running child and poll try_wait
at bounded intervals until its reaped status is observed. Never use an unbounded
wait/wait_with_output, assume Child Drop kills, or report success after merely
sending a signal. Assertions run after cleanup, including early fixture errors.
Timeout/error in kill/reap is test failure, never simulated process-death evidence;
the existing job timeout remains an outer runner safeguard. This is not a promise
to recover from host/kernel failure. TASK-012 repeats applicable attacks through
its real managed launch/sandbox path and keeps its independent process limits.

## 8. Canonical acceptance candidates

Before start, the Specification must become the sole normative owner of these
exact scenarios. The next unused IDs after TASK-010 are proposed.

```gherkin
AC-101
Given a host-bound private Plugin session and the frozen mengxia.plugin.v1 descriptor
When the Plugin completes version/challenge negotiation and ping correlation
Then only the private protocol types are accepted
And no Core Client/Admin operation, actor claim, DB/CAS path or authority-bearing handle is representable.

AC-102
Given frame, decode, queue, session, stderr and deadline inputs at cap-1, cap and cap+1
When a Plugin session processes or rejects them
Then memory and concurrency remain within the accepted checked limits
And backpressure, timeout, cancellation, abandonment and shutdown close all TASK-011-owned concurrent work and release admission without an unbounded queue or detached wait.

AC-103
Given the test-only hostile Plugin emits malformed, oversized, unknown, duplicated, flooded, truncated, crashing or hanging behavior
When the conformance harness exercises the private protocol
Then the host fails closed with the exact safe error class and no untrusted diagnostic disclosure
And the harness terminates and reaps its test process within its bounded timeout.
```

TASK-011 must not claim AC-020..AC-028. In particular, AC-021/AC-022 require the
real TASK-012 sandbox and AC-023 requires later Broker composition.

## 9. Stable test candidates and direct command ownership

Each ID must execute its complete obligation directly in
`scripts/verify-task-011.sh`; passing a nearby test or a later workspace sweep is
not evidence for an omitted mapping.

| Stable ID | Complete obligation |
|---|---|
| `TEST-PROTO-011` | exact source/descriptor/provenance/package/tag/reservation/generation hashes and frozen golden messages |
| `TEST-WIRE-011` | unknown/reserved/duplicate/nonminimal/wrong-wire/group/depth/truncation closed scanner matrix |
| `TEST-AUTHORITY-011` | descriptor and Rust graph cannot represent/import Core/Admin/actor/DB/CAS/path/credential authority |
| `TEST-BOUNDS-011` | every numeric cap invalid/cap-1/cap/cap+1 plus checked aggregate accounting; depth 1 rejected and depth 2 valid handshake; cap equality follows §4 |
| `TEST-QUEUE-011` | full inbound/outbound queues apply backpressure with no drop/unbounded side buffer |
| `TEST-STDERR-011` | concurrent drain, scratch-buffer cap and total cap; raw diagnostics discarded and absent from errors/logs |
| `TEST-DEADLINE-011` | end-to-end handshake/request/shutdown budgets including pending write/flush/full queue; deadline propagation, partial-write closure and cancellation races have one terminal result |
| `TEST-LIFECYCLE-011` | permit/open, sequence, EOF, valid/invalid failure, shutdown and caught-panic matrix; driver/request drop and caller-owned abort/await release every endpoint and permit |
| `TEST-HOSTILE-011` | every closed fake action runs as a real test child and is boundedly reaped |
| `TEST-ARCH-011` | exact Cargo edges and forbidden production symbols/dependencies |
| `TEST-SUPPLY-011` | §6.1 current graph and historical-fixture compatibility; version/feature/source/license/advisory plus descriptor regeneration from pinned artifact |
| `TEST-DOC-011` | accepted proposal/version/status/AC/TEST/file-scope/start-record exclusivity and version data; rejects stale current NONE/BLOCKED text during activation and requires NONE again at completion |

`TEST-HOSTILE-011` owns real child-process evidence. In-memory stream tests cannot
silently replace it. `TEST-LIFECYCLE-011` owns structured session cleanup but
does not claim production OS-process join.

### 9.1 Additive CI attribution under ADR-0015

Keep scripts/ci-baseline-mappings.txt byte-identical (150 retained IDs) and keep
the five MAINT-003 obligations. Add scripts/ci-task-011-mappings.txt containing
exactly the twelve IDs above, once each. It is code-reviewed executable evidence
input, not a document-only exemption. Validate its set against actual top-level
TASK-011 mappings, reject duplicates/unknown/comment-only/zero-test mappings, and
require disjointness from both retained sets. ci_evidence.rs retains its historical
baseline test and adds this independent set plus missing/failed-result negatives.

verify-task-011.sh supports developer|formal with optional component or
native-component, using scripts/ci-evidence.sh. Native mode executes all task-local
checks, including descriptor regeneration, and emits only COMPONENT_PASS; shared
advisory/license/source policy is explicitly REQUIRED_BY_AGGREGATE through
ci_supply. Standalone developer/formal commands run shared supply before their
FAST_PASS/PASS labels. No new ci_run_group group or skip switch is introduced.

verify-repository.sh invokes TASK-011 exactly once in native-component mode.
Its local complete aggregate and ci.yml's existing Merge gate add the twelve new
labels only after their existing required checks succeed. The hosted aggregate
must contain exactly 167 distinct IDs (150 + 5 + 12), and the local aggregate 166
because real second UID remains hosted separately. formal-native alone cannot
print terminal PASS. Existing checkout SHA/run/attempt identity, supply, real UID,
fast feedback and dependency review checks stay mandatory in their existing event
matrix. Preserve all failure propagation; a failed TASK-011 component must prevent
repository/merge PASS. Do not alter check-ci-merge-gate.sh's event/result policy.

Extend the workflow's attribution loop and display name only; job graph, runners,
pins, permissions, attestation, triggers, classification and protections stay
unchanged. Add tests for absent/duplicate new IDs, native versus complete labels,
component failure and the exact final union. Existing CI tests/shell fixtures that
copy execution inputs must include the new additive mapping file where needed.

## 10. Exact candidate file scope

Only after independent acceptance and a synchronized canonical start record may
TASK-011 modify:

```text
Cargo.toml
Cargo.lock
AGENTS.md
crates/mengxia-plugin-proto/Cargo.toml
crates/mengxia-plugin-proto/build.rs
crates/mengxia-plugin-proto/src/lib.rs
crates/mengxia-plugin-proto/src/wire.rs
crates/mengxia-plugin-proto/src/codec.rs
crates/mengxia-plugin-host/Cargo.toml
crates/mengxia-plugin-host/src/lib.rs
crates/mengxia-plugin-host/src/limits.rs
crates/mengxia-plugin-host/src/session.rs
crates/mengxia-plugin-host/src/error.rs
crates/mengxia-testkit/Cargo.toml
crates/mengxia-testkit/src/bin/task_011_hostile_plugin.rs
crates/mengxia-testkit/tests/architecture.rs
crates/mengxia-testkit/tests/ci_orchestration.rs
crates/mengxia-testkit/tests/ci_evidence.rs
crates/mengxia-testkit/tests/document_traceability.rs
crates/mengxia-testkit/tests/naming.rs
crates/mengxia-testkit/tests/task_010_foundation.rs
crates/mengxia-testkit/tests/task_011_foundation.rs
crates/mengxia-testkit/tests/support/plugin_supply.rs
crates/mengxia-testkit/tests/support/plugin_hostile.rs
crates/mengxia-testkit/tests/fixtures/task_011/**
proto/plugin/v1/control.proto
proto/plugin/v1/control.pb
proto/plugin/v1/control.provenance
scripts/verify-proto-artifacts.sh
scripts/verify-task-010.sh
scripts/verify-task-011.sh
scripts/verify-repository.sh
scripts/ci-task-011-mappings.txt
.github/workflows/ci.yml
docs/proposals/TASK-011-GATE-PROPOSAL.md
docs/spec/adr/ADR-0017-task-011-private-plugin-protocol.md
docs/spec/IMPLEMENTATION_SPEC.md
docs/spec/DECISIONS.md
docs/spec/IMPLEMENTATION_REVIEW.md
docs/spec/IMPLEMENTATION_PLAN.md
docs/spec/PROJECT_INTAKE_REPORT.md
docs/spec/task-lifecycle-records.toml
```

Cargo.lock changes only the three expected local package blocks under §6.1;
third-party blocks and features stay unchanged. Cargo.toml adds only the path
declarations in §6. No deny.toml change is authorized. mengxia-framing is
intentionally omitted: its accepted codec already
supports a `FrameLimit` inside this proposal's range, rejects before allocation and
does not need Plugin-specific behavior.

The CI workflow change is limited to its display name and §9.1 final evidence
attribution. The repository driver adds exactly one non-recursive TASK-011 native
component and the corresponding local aggregate. The new mapping file and
ci_evidence.rs changes prove additive ownership; they cannot bypass supply,
second UID or any retained ID. Existing job topology, action pins, permissions
and attestation do not change. scripts/ci-evidence.sh and the historical mapping
file are reused unchanged.

The two TASK-010 file exceptions permit only §6.1 supply-test compatibility and
their direct mapping assertions; they do not reopen package/security product
behavior. task-lifecycle-records.toml changes only the five document-version
scalars as necessary, not MAINT-002 status, authority or historical evidence.
The shared supply and hostile-harness helpers are test-only. The enumerated
wire/codec/limits/session/error modules are optional organization within the same
protocol boundary, not extra deliverables or authority. This permits a modular
implementation without forcing everything into lib.rs or reopening file scope.
Other files are not implicitly authorized by a crate-directory prefix.

Explicitly forbidden files/surfaces include Core proto and its frozen fixtures,
app, ports, domain/events, store/migrations, storage, platform filesystem/sandbox,
CLI, daemon and all schemas from TASK-010.

## 11. Accepted canonical synchronization

The accepted atomic documentation change:

1. accept ADR-0017 and the exact cap table, closing only the TASK-011 protocol/log/
   session sub-scope of `OQ-006`;
2. keep TASK-012 OS-process/sandbox resource caps and all Provider/release portions
   of `OQ-006` open;
3. add the typed keys and their tightening-only semantics to Specification §16;
4. add AC-101..AC-103 as bare canonical Gherkin IDs and the twelve TEST rows as
   canonical tables;
   remove duplicated normative scenario/obligation prose from this supplement
   when adopted, leaving exact ID references and implementation detail only;
5. change only TASK-011 from `BLOCKED/NONE` to
   `IN_PROGRESS/TASK_011_PRIVATE_PROTOCOL_ONLY` in Specification, Plan, Review,
   Intake and AGENTS;
6. add a document test that requires every positive marker and rejects stale
   contradictory `TASK-011 unauthorized`, `BLOCKED` and authority `NONE` text in
   current-state sections while preserving clearly historical statements; it must
   also allow DONE with authority NONE and require reviewed completion evidence,
   so finishing the task does not require rewriting the lifecycle test;
7. creates the active start record below after independent review passed.

Synchronize the five version scalars in task-lifecycle-records.toml with the
canonical headers/references in this same change. Adopt §6.1's narrow replacement
of historical whole-current-lock assertions and §9.1's additive CI attribution in
ADR-0017/Decisions explicitly; preserve ADR-0014's pure package boundary and
ADR-0015/0016's mandatory checks. Do not rewrite historical completion records.

Canonical authority remains conditional on all positive and negative document
tests passing together; prose alone cannot activate a task.

## 12. Authorized implementation order after acceptance

```text
STEP-1  Apply §6's final normal/build/dev edges using the retained lock. Inspect
        the exact three-block delta; add historical fixture and §6.1 TASK-010
        compatibility checks in the same work batch. Run locked/offline metadata,
        affected-package build and supply checks before implementing protocol code.
STEP-2  Commit exact plugin proto source, descriptor and provenance; extend the
        pinned protoc regeneration check and freeze golden vectors.
STEP-3  Implement descriptor-derived closed wire scanning and generated types.
STEP-4  Implement typed immutable limits, admission and the caller-supplied-stream
        session state machine with one in-flight request.
STEP-5  Implement bounded stdout/control queues, concurrent bounded stderr and
        §5.6 structured drop/cleanup; retain termination as a typed request only.
STEP-6  Add the test-only hostile executable and complete conformance harness.
STEP-7  Wire each stable ID to its complete command; update architecture, naming,
        CI orchestration and §9.1 additive native/local/hosted evidence paths.
STEP-8  Run task, workspace, all-target/all-feature, Clippy, docs, developer,
        supply and diff/scope gates; review the complete diff.
STEP-9  Open a code PR. Review formal macOS, real second UID, shared supply,
        applicable fast/dependency-review and exact Merge gate evidence. Review
        CodeQL where the existing public-repository policy supports the event;
        do not invent a globally required check that excludes fork contributions.
        Verify all 167 hosted IDs and their exact checkout/run/attempt attribution.
STEP-10 After reviewed PR and exact merged-main evidence pass, record AC-101,
        AC-102, AC-103 and each of the twelve stable IDs individually; revoke
        authority to NONE. Do not start TASK-012 automatically.
```

Any dependency outside §6, public operation, real production process spawn, executable
path, persistence, migration, sandbox claim or file outside §10 stops work and
returns to review.

## 13. Active start record

```text
TASK011_CANONICAL_GATE: ACCEPTED
TASK011_LIFECYCLE: IN_PROGRESS
TASK011_IMPLEMENTATION_AUTHORITY: TASK_011_PRIVATE_PROTOCOL_ONLY
TASK011_PROPOSAL_VERSION: 0.1.2
TASK011_ADR: ADR-0017_ACCEPTED
TASK011_OQ006: TASK011_PROTOCOL_LOG_SESSION_CAPS_ACCEPTED_LATER_SUBSCOPES_OPEN
TASK011_ALLOWED_SCOPE: proposal §10 exactly
TASK011_FORBIDDEN: install/grant/revoke/activation/executable-custody/production-spawn/kill/sandbox/Broker/Core/Admin/DB/CAS/persistence/migration/CLI/daemon/TASK-012+
TASK011_ACCEPTANCE: AC-101,AC-102,AC-103
TASK011_TESTS: IMPLEMENTATION_SPEC.md exact TASK-011 registry
TASK011_BASELINE: exact reviewed head and accepted proposal dependency preflight
TASK011_FEATURES: FUNC-006; FUNC-007
TASK011_REQUIREMENTS: API-001; API-002; API-004; SEC-005; SEC-017; SEC-020; SEC-021; REL-001; REL-006; CFG-001; CFG-003
```

## 14. Independent review checklist

- caps have exact units, inclusivity, counting, checked arithmetic and cap-1/cap/
  cap+1 evidence;
- queue capacity bounds both memory and admitted work; no hidden retry/log channel;
- stderr is concurrently drained into bounded scratch space, counted, discarded
  and absent from safe diagnostics;
- `prost` is preceded by a closed scanner; unknown/duplicate/noncanonical data
  cannot disappear silently;
- schema has exact package/tags/reservations, one in-flight request and no authority
  or arbitrary payload field;
- challenge is cross-wire detection only and never identity/authorization;
- production host accepts streams but cannot spawn, locate or kill an executable;
- structured driver drop closes owned resources without detached tasks; the
  caller's outer task and TASK-012's process custody remain explicitly owned;
- real hostile child exists only in testkit; every successful harness result
  proves reap, and cleanup failure never becomes PASS;
- TASK-012 retains process/sandbox/resource terminal ownership;
- dependency kinds distinguish runtime/build/test tools; no forbidden runtime
  Core/app/ports/store/platform edge or new network authority exists;
- no new registry dependency/version/feature or deny-policy relaxation exists;
- Core v1.3 proto and completed product behavior remain unchanged; the enumerated
  TASK-010 test compatibility exception retains every prior supply obligation;
- stable test commands directly cover their named obligation;
- exact file whitelist includes all implementation and mechanical gate changes;
- current and historical authority statements are distinguishable and consistent.

## 15. Current next action

```text
READINESS: READY_TO_IMPLEMENT_TASK_011
BLOCKERS: NONE
NEXT_SAFE_ACTION: proposal §12 STEP-9 code PR and reviewed formal evidence
PRODUCTION_CODE_CHANGE: TASK_011_PRIVATE_PROTOCOL_ONLY
START_RECORD: ACTIVE
```

## 16. v0.1.2 correction and verification record

This section records the repaired candidate that preceded independent approval.
The v0.1.1 six review findings are addressed in §4–§6.1, §9.1 and §10–§12: legacy-lock
compatibility, additive CI/version data, pre-write deadlines, drop ownership,
PluginFailure outcomes, and exact depth/stderr equality. The same pass corrects
build-dependency kinds, real-child transport without a new Tokio feature, protocol
ack versus process-exit ownership, and PR/merged-main completion evidence.
Architecture/strong-contract conflicts were recorded as pending recommendations
in DECISIONS.md; at that pre-acceptance checkpoint implementation authority was NONE.

Dependency preflight used an isolated copy of reviewed head
1260bf99dbc9e9bd0af1715510d12c27d2f8ffd7, not the working repository manifests.
Broad offline lock regeneration selected newer cached transitive versions and was
rejected. Repeating resolution from the original lock with §6's exact manifests
changed only plugin-host/plugin-proto/testkit blocks. Product code, current lock,
Core/schema/migrations and existing CI execution were not changed by this repair.
An isolated Rust transport probe compiled and ran with the candidate locked graph:
three std UnixStream pairs were inherited as child stdin/stdout/stderr, existing
Tokio net/io features exchanged control and diagnostics concurrently, and try_wait
observed the child's reaped exit. No Tokio process feature, unsafe FD conversion
or additional third-party dependency was used. This proves the harness adapter
route, not the future protocol scanner or hostile-action matrix.
The corrected working-tree document gate passed all 22 current tests, and
verify-ci-fast.sh passed workspace format/check/Clippy plus its build-boundary,
CI and toolchain checks as non-formal feedback. Draft-only contract checks
confirmed 12 disjoint stable IDs, 42 unique scoped paths, numeric accounting and
version consistency. The dependency comparison retained all 157 lock packages;
diff checks confirmed no product/lock/CI execution changes. These are local evidence,
not a self-issued independent approval.
Executable TASK-011 conformance and hosted acceptance remain implementation work;
passing the start document gate cannot substitute for completion evidence.

Version 0.1.2 additionally aligns the existing canonical
`MENGXIA_PLUGIN_LOG_BYTES` name with its per-session cumulative quota semantics;
the 8192-byte discard-only drain scratch cap remains a fixed ADR-versioned safety
constant rather than a misleading configuration source. It also closes TASK-011's
test-fixture process fan-out at exactly one live, sequentially owned and reaped
child. TASK-012 retains all production process and process-tree limits.

## 17. Local implementation checkpoint

The exact §10 implementation scope is locally complete and remains `IN_PROGRESS`.
`./scripts/verify-task-011.sh developer` passed all twelve directly owned stable
IDs, exact protoc regeneration, workspace format/Clippy/tests, shared supply and
diff checks. The frozen source and descriptor digests are respectively
`4e6cd9898db0c3937c53d9bfd51ec5b3c330bfc7afa3914e6006b459090eee84`
and `b5088d5da6e01671322234a1491cf7d69c0e59488be12540301228053ff73a52`.
The historical TASK-010 lock fixture remains
`302df8141acee77aa58ecb796a53ecbb4faf9f6cd55dc384667bb08e725c0b2e`;
the current lock changes only the three authorized local package blocks and has
digest `3c30cce5ad8babd9b494995e357ae5b419bc17cd2cdf979aa36241433737f22c`.

This checkpoint is developer evidence only. It does not mark AC-101..AC-103 or
the stable IDs formally complete, revoke authority, or authorize TASK-012. The
next action is §12 STEP-9: create the code PR and review its exact formal macOS,
second-UID, shared-supply and 167-ID Merge gate evidence.
