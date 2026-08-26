# TASK-003 start-gate proposal

> Status: **ACCEPTED / INCORPORATED BY CANONICAL SPECIFICATION v1.1.16**
>
> Date: 2026-08-26
>
> Scope: accepted normative TASK-003 implementation supplement. The synchronized
> canonical start record authorizes only §4 and preserves every listed forbidden
> boundary and later-task gate.

## 1. Current outcome

`TASK-004` is complete and supplies the durable Library owner/lock prerequisite
selected by Option A. The user accepted the exact contracts in §§4–11, including the
wire, lifecycle, composition, runtime namespace, CLI, dependency, error-taxonomy and
AC/TEST ownership conflict resolutions. Specification v1.1.16, the Decision Log,
Review, Plan, Intake and `AGENTS.md` synchronize that acceptance and the exact start
record makes `TASK-003` alone `IN_PROGRESS`.

## 2. Evidence read

- Canonical Specification v1.1.14, especially §0.5, `API-001`..`API-003`,
  `API-008`..`API-010`, `SEC-005`, `SEC-013`, `SEC-014`, `SEC-017`, `SEC-020`,
  `SEC-021`, `REL-001`, `REL-006`, `CFG-001`, `CFG-003`, §§10.1, 12.3, 16, 17
  and TASK-003.
- Decisions v0.3.14, including `BASE-007`, `BASE-008`, `BASE-012`..`BASE-014`,
  `BASE-016`, `BASE-017`, `DEC-007`, `DEC-012`, `DEC-016`, `DEC-017`,
  `DEC-019`, `DEC-021`, `DEC-022`, ADR-0001, ADR-0004 and ADR-0005.
- At gate drafting, Review v1.1.24 and Plan v0.3.24 recorded TASK-001, TASK-002 and
  TASK-004 as `DONE`, with TASK-003 `PENDING` until this stable registry and start
  record were accepted. The synchronized canonical documents now supersede that
  historical lifecycle snapshot and set TASK-003 alone to `IN_PROGRESS`.
- The reviewed pre-start repository baseline was commit `596374f`. At that baseline,
  `mengxia-core-proto`, `mengxia-framing`, `mengxiad` and `mengxia` were empty
  skeletons and no proto source, IPC, CAS, TCP or product command existed. This is
  historical intake evidence, not a requirement that the authorized implementation
  remain absent.
- Formal CI run `32912547078` proved that macOS 26 provides the required executable
  test utility at `/bin/test`, while `/usr/bin/test` does not exist. The previously
  accepted absolute path was `SPEC_STALE`; Specification v1.1.16 corrects only that
  test-preflight path and changes no production authority or runtime behavior.
- At the pre-start baseline, `mengxia-store-sqlite` exported only configuration and
  `StoreError`.
  `OpenedLibraryOwner`, `StoreHandle`, `OpenedLibraryMetadata`, bootstrap/open entry
  points and their owner/library identity are all `pub(crate)`. A daemon cannot yet
  consume the completed durable owner/lock context without an explicitly authorized
  narrow composition API.
- TASK-004's exact canonical namespace accepts only its fixed SQLite/lock/recovery
  names. A Client socket or runtime directory cannot be placed in the Library root
  without violating AC-067/AC-070 and reopen validation.
- Workspace `tokio` is already exactly 1.53.1 with `sync` only. Its checked-in crate
  source exposes safe `UnixStream::peer_cred()` and uses `getpeereid` plus
  `LOCAL_PEEREPID` on macOS; MengXia IPC code needs no unsafe FFI for peer identity.
- Official `prost`/`prost-build` 0.14.4 metadata records Apache-2.0, MSRV 1.85 and
  descriptor-based `compile_fds`; normal `compile_protos` requires `protoc`.
- Canonical Specification §16 fixes `MENGXIA_MAX_DECODE_DEPTH` at default 64 with
  a tightening-only range of 1..=64. The locked `prost` 0.14.4 source instead fixes
  its private `RECURSION_LIMIT` at 100 and states that `DecodeContext` cannot be
  customized. Relying on `Message::decode` alone therefore cannot prove CFG-003.
- Official Protocol Buffers release `v35.1` publishes
  `protoc-35.1-osx-aarch_64.zip` with SHA-256
  `193289af0470c6a1aada357d4fba0bbf8d78bfaac8b5e42ca30af2ef75583de2`.
  Release source: <https://github.com/protocolbuffers/protobuf/releases/tag/v35.1>.
- The local host has no ambient `protoc`. Default local development must therefore
  remain independent of a compiler download or PATH discovery.

## 3. Stable blocker disposition

### `TASK003-BLOCKER-001` — durable owner authority unavailable

- Category: `ARCHITECTURE`
- Severity: `CRITICAL`
- Scope: `TASK_SPECIFIC`
- Owner: `USER_DECISION_REQUIRED`
- Status: **RESOLVED**
- Resolution: the user accepted Option A; TASK-004 is `DONE` and owns first-create,
  durable Library owner persistence and the retained Library lock (`BASE-017`).
- Remaining constraint: IPC consumes an already-open context and never reads SQLite,
  persists owner authority or infers it from eUID/environment/CLI/request data.

### `TASK003-BLOCKER-002` — public wire contract is incomplete

- Category: `SPECIFICATION`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: the canonical task says “framed proto3” but does not define exact message
  fields/numbers, a response discriminant, reserved actor evidence, validation order,
  version selection or connection completion.
- Minimum action: accept or revise §5 and publish it canonically.

### `TASK003-BLOCKER-003` — bounded listener/handshake lifecycle is incomplete

- Category: `SECURITY`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: the frame cap is accepted, but timeout, concurrent handshake capacity,
  admission, cancellation, shutdown ordering and terminal connection behavior are not.
- Minimum action: accept or revise §6 and the two finite cap values in §8.

### `TASK003-BLOCKER-004` — offline proto-generation policy is incomplete

- Category: `DEPENDENCY`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: ambient `protoc` is absent and forbidden as an implicit build dependency;
  the prior draft selected neither an exact compiler artifact nor a normal-build
  boundary that keeps local/offline Cargo usable.
- Minimum action: accept or revise §9's descriptor-first policy and exact v35.1 pin.

### `TASK003-BLOCKER-005` — stable AC/TEST registry and start record are absent

- Category: `SPECIFICATION`
- Severity: `CRITICAL`
- Scope: `TASK_SPECIFIC`
- Owner: `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: canonical documents define only AC-028/AC-029 for TASK-003 and no stable
  TASK-003 TEST registry or active start record.
- Minimum action: after §§5–10 are accepted, publish §11's exact AC/TEST definitions,
  synchronize all lifecycle documents, run traceability and create one start record.

### `TASK003-BLOCKER-006` — no consumable opened-Library composition seam

- Category: `REPOSITORY`
- Severity: `CRITICAL`
- Scope: `TASK_SPECIFIC`
- Owner: `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: the completed store owner/lock lifecycle is crate-private. Neither daemon
  nor IPC can obtain a typed owner UID while retaining the exact lock lifetime.
- Minimum action: accept §7's narrow opaque API and add its store files to TASK-003
  scope. Do not expose SQLite connections, rows, SQL, paths or raw lock handles.

### `TASK003-BLOCKER-007` — runtime endpoint ownership and stale cleanup are undefined

- Category: `SECURITY`
- Severity: `CRITICAL`
- Scope: `TASK_SPECIFIC`
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: the socket cannot live in the exact TASK-004 Library namespace, while a
  pathname, UID and mode alone do not prove that an existing same-name socket is a
  stale MengXia endpoint. Blind unlink could delete another live/user-created socket.
- Minimum action: accept or revise §8's separate versioned runtime namespace,
  ownership marker, liveness probe and fail-closed cleanup protocol.

### `TASK003-BLOCKER-008` — production configuration owner is incomplete

- Category: `SPECIFICATION`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: TASK-003 must resolve Library root, Client endpoint and IPC caps in the
  composition root, but the repository has no production layered resolver. Reading
  environment directly in framing/store would violate §16 and AC-072.
- Minimum action: accept §8's immutable DTO/resolver boundary and exact source rules.

### `TASK003-BLOCKER-009` — configurable decode-depth enforcement is absent

- Category: `SPECIFICATION`
- Classification: `CONFLICT`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: canonical CFG-003 requires the typed, tightening-only
  `MENGXIA_MAX_DECODE_DEPTH` boundary before TASK-003 decodes protobuf input, but the
  prior draft omitted the key and selected `prost` 0.14.4, whose internal recursion
  limit is fixed at 100 and has no public per-decode override. A 65-level group or
  reachable nested-message payload could therefore cross the accepted 64-level
  boundary before application validation.
- Minimum action: accept §5.1's descriptor-derived, allocation-free wire-depth
  preflight and §8.1's exact config/startup behavior; do not enable Prost's
  `no-recursion-limit` feature or claim its fixed 100-level guard implements CFG-003.
- Verification: descriptor-cycle/depth checks and depth 1/2/3/63/64/65 plus nested
  group/malformed-length fixtures prove rejection occurs before Prost decode and
  before Library/runtime namespace mutation.

### `TASK003-BLOCKER-010` — partial marker staging has no cleanup provenance

- Category: `SECURITY`
- Classification: `CONFLICT`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: a fixed staging name plus regular type, owner UID, mode, empty ACL, link
  count and bounded length cannot prove that a zero-byte or partially written file was
  created by MengXia. Automatically unlinking such a file could destroy pre-existing
  user data in an otherwise accepted owner-only runtime directory.
- Minimum action: accept §8.2's fail-closed rule: only a complete, typed,
  checksum-valid staging record matching the opened Library identity may continue
  no-replace publication; zero/partial/invalid or otherwise unproven staging is
  retained unchanged and never automatically removed or recreated.
- Verification: real and fault-seam crash cases prove zero/partial staging bytes and
  inode identity remain unchanged across reopen, no link/unlink/bind occurs, and
  startup returns one redacted `STORAGE_CONFIGURATION_ERROR` before IPC admission.

### `TASK003-BLOCKER-011` — CLI/daemon public behavior is incomplete

- Category: `SPECIFICATION`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: TASK-003 is the first task permitted to implement `mengxiad` and `mengxia`,
  but the canonical task fixes neither their command grammar nor request-ID source,
  output, exit status or signal/shutdown behavior. Implementing the binaries from the
  wire schema alone would silently invent a public interface and leave
  `TEST-CLI-001` without deterministic observable assertions.
- Minimum action: accept or revise §8.3's two-command public contract and publish it
  canonically before either binary is implemented.
- Verification: CLI subprocess tests prove the exact grammar, precedence, generated
  request ID, bounded output, redaction, exit-status mapping, response-less close and
  joined signal shutdown without Library/runtime mutation on parse/help failures.

### `TASK003-BLOCKER-012` — local IPC outcomes do not fit the accepted error registry

- Category: `SPECIFICATION`
- Classification: `CONFLICT`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`
- Status: **RESOLVED**
- Problem: canonical §14.1 limits `STORAGE_IO_ERROR` to filesystem/backend failures
  and `UNSUPPORTED_CAPABILITY` to declared Provider/Plugin capability contracts, while
  the prior TASK-003 draft reused them for Client socket transport and protocol-version
  negotiation. Connect/write/flush/read/reset outcomes also had no complete stable
  mapping. Implementing that draft would silently change two accepted error meanings.
- Minimum action: accept §10's explicit taxonomy conflict resolution and add the three
  exact TASK-003 registry rows before IPC implementation.
- Verification: exhaustive `ErrorCode` round-trip/source/metric fixtures plus Client
  subprocess and handshake matrices prove transport, malformed protocol, incompatible
  version and deadline are distinct and never leak an OS or peer diagnostic.

### Completion-only evidence gap — real unauthorized UID

- Classification: `EXPECTED_GAP`, not a start blocker once the obligation is stable.
- Local evidence can prove real same-UID credential retrieval plus deterministic
  mismatch policy. The production owner-only hosting/final directories and socket
  intentionally prevent a second UID from reaching `accept`; a real second UID MUST
  first prove that production-path connection fails at the OS boundary with no server
  admission or payload read.
- The same reviewed ephemeral CI run MUST also exercise the server mismatch branch
  through a private `#[cfg(test)]` listener fixture. That fixture may relax only its
  disposable test socket's directory/socket reachability so the real second UID can
  connect; it MUST pass the resulting accepted `UnixStream` to the exact production
  peer-credential and pre-frame handshake function, retain the durable Library owner
  UID as the expected identity, and prove rejection before header read, allocation,
  correlation-ID generation or state construction.
- The formal `macos-26` job creates exactly one disposable local account named
  `mengxia-task003-ci` before running this fixture. The name must be absent initially;
  the job selects the first unused decimal UID in the closed range 600..=699 from one
  bounded `/usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -list /Users UniqueID`
  snapshot, creates the account only through absolute non-interactive `sudo` plus
  cleared-environment `dscl` commands with primary GID 20,
  `/var/empty` home and `/usr/bin/false` shell, then re-reads and proves its numeric UID
  is the selected value, nonzero and unequal to the owner-role eUID. A trap deletes
  only that exact account through the final command below on every
  success/failure exit; collision, no free UID, parse ambiguity, creation/revalidation
  failure or cleanup failure fails the formal job. The cleanup trap is installed before
  the first account-creation command. No password, AuthenticationAuthority, home
  directory, group or network identity is created.

  ```text
  /usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci
  /usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci UniqueID <selected-ASCII-UID>
  /usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci PrimaryGroupID 20
  /usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci NFSHomeDirectory /var/empty
  /usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci UserShell /usr/bin/false
  /usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -delete /Users/mengxia-task003-ci
  ```

  No `sysadminctl`, interactive sudo, password/hash field, dynamic account name or
  unbounded UID search is permitted.
- After account revalidation, the privileged runner executes exactly one owner-role
  Rust test command; that test internally performs both production-path denial and
  permissive-fixture peer rejection with separate joined child invocations:

  ```text
  cargo test -p mengxiad --bin mengxiad --locked --offline task_003_real_second_uid_peer_is_rejected_before_frame -- --exact --ignored --nocapture
  ```
- The fixture is one private unit-test entry compiled only under `#[cfg(test)]`. Its
  owner-role process creates the unique disposable permissive endpoint beneath a
  dedicated `/private/tmp` test directory and invokes the same current test executable
  through this exact argv prefix:

  ```text
  /usr/bin/sudo -n -u mengxia-task003-ci -- /usr/bin/env -i
  ```

  Before invocation, the fixture owner role uses the same exact sudo
  target plus absolute `/bin/test -x <absolute-current-test-executable>` to prove
  every path prefix is searchable and the executable is readable/executable by that
  account; it never chmods/chowns a repository or build artifact to make the preflight
  pass.
  The child role is selected only inside that test module by
  `MENGXIA_TASK003_TEST_ROLE=second_uid_client` and receives only
  `MENGXIA_TASK003_TEST_ENDPOINT=<exact fixture path>`; all other environment is
  cleared. The subprocess selects the fixed ignored unit test
  `task_003_real_second_uid_peer_is_rejected_before_frame` with exact Rust test-harness
  `--exact ... --ignored --nocapture` arguments. The child performs one bounded canary
  write and never uses the production CLI, an ambient client executable, shell
  discovery or a network listener. The owner process joins the child,
  revalidates/removes only its exact fixture inode and fails if readiness, child exit
  or cleanup exceeds the accepted test deadline.
- The two environment names, fixed CI account name and role branch MUST be absent from
  every non-test Rust artifact; the account name may otherwise occur only in the two
  checked formal scripts. They are test-binary/runner inputs, not production
  configuration or a Cargo feature. `TEST-ARCH-003` checks that the test entry/client
  branch cannot be linked or selected by `mengxiad`, `mengxia` or any production
  library API.
- TASK-003 may not become `DONE` or claim supported unauthorized-peer denial until
  both real-UID cases pass in the same reviewed formal run. A synthetic UID alone is
  insufficient, and the local developer gate must not require privileged host mutation.

## 4. Scope boundary

Proposed TASK-003 scope is limited to:

- `proto/core/v1/handshake.proto` and committed descriptor evidence;
- `crates/mengxia-core-proto/**`;
- `crates/mengxia-framing/**`;
- `bins/mengxiad/**` and `bins/mengxia/**` for exactly the two handshake-only public
  commands in §8.3;
- the narrow opened-Library composition API described in §7 inside
  `crates/mengxia-store-sqlite/**`;
- the narrow runtime-directory/socket authority described in §8 inside
  `crates/mengxia-platform-fs/**`;
- `crates/mengxia-types` and retained TASK-002 error fixtures/scripts only for the
  proposed `IPC_TRANSPORT_ERROR`, `PROTOCOL_VERSION_UNSUPPORTED` and
  `DEADLINE_EXCEEDED` registry variants and exhaustive safe-code checks;
- exact Cargo/lock/deny updates, TASK-003 scripts/tests/fixtures, reviewed CI wiring
  for the unprivileged normal gate and privileged second-UID test, including only the
  private `#[cfg(test)]` reachability fixture defined above, and synchronized lifecycle
  documents.

Forbidden:

- product Commands/Queries, `CommandRecord`, audit/event writes or migration `0001+`;
- Admin listener/session/handler, Plugin/Broker channel, CAS, Provider, Credential,
  HTTP/TCP/loopback listener, remote transport or multi-tenant claim;
- public SQLite connection/row/SQL/path/lock APIs;
- caller-supplied actor/role/UID, owner inference, detached tasks, unbounded frame,
  unbounded/dependency-default decode depth, unbounded wait/retry, ambient `protoc`,
  ambient executable discovery or runtime network download.

## 5. Proposed exact wire contract

The canonical source uses `syntax = "proto3"` and exact package
`mengxia.core.v1`. TASK-003 publishes only handshake messages; it does not publish a
placeholder product command or opaque operation bytes.

```proto
message ClientHello {
  string request_id = 1;
  uint32 protocol_major = 2;
  reserved 3;
  reserved "actor", "actor_principal";
  uint32 min_protocol_minor = 4;
  uint32 max_protocol_minor = 5;
}

message ServerHello {
  string request_id = 1;
  string correlation_id = 2;
  uint32 protocol_major = 3;
  uint32 protocol_minor = 4;
}

message ErrorEnvelope {
  string code = 1;
  string safe_message = 2;
  bool retryable = 3;
  optional string correlation_id = 4;
  map<string, string> safe_details = 5;
}

message HandshakeResponse {
  oneof response {
    ServerHello hello = 1;
    ErrorEnvelope error = 2;
  }
}
```

- Every message uses exactly four unsigned big-endian payload-length bytes followed
  by that many protobuf bytes. Length excludes the header, zero is invalid, and the
  configured maximum is inclusive.
- The reader consumes the complete header first, rejects zero/cap+1 before payload
  reservation/allocation, uses fallible bounded allocation, and never resynchronizes
  after malformed/truncated input. Media bytes remain forbidden.
- After peer authentication, the server reads exactly one frame and decodes it only
  as `ClientHello`. Unknown fields, including an injected wire tag 3, never become
  authority. Missing/invalid fields fail validation.
- TASK-003 supports only major 1, minor 0. A hello is compatible only when
  `protocol_major == 1`, `min_protocol_minor <= max_protocol_minor` and the inclusive
  range contains 0. The server selects 0. No common version returns
  `PROTOCOL_VERSION_UNSUPPORTED`.
- `request_id` is exactly the canonical UUIDv7 text accepted by `mengxia-types`.
  `correlation_id` is a fresh fallible server-generated UUIDv7; generation failure
  returns `ID_GENERATION_UNAVAILABLE` without a fabricated ID.
- `ServerHello` echoes the request ID but never returns UID, GID, PID, role, Library
  path or Admin status. Safe errors contain only registry strings/static messages;
  raw frame bytes, path, UID/GID/PID and OS diagnostics are never reflected.
- For negotiated major 1/minor 0, `safe_details` MUST be empty on every server
  response and the Client rejects a non-empty map as `VALIDATION_ERROR`. The field is
  retained at its canonical number only for a future protocol version with its own
  accepted key registry and entry/key/value bounds; TASK-003 does not invent them.
- Missing/mismatched peer credentials close without reading a frame or sending an
  oracle-bearing response. Under negotiated major 1/minor 0, an authenticated peer
  may receive at most one bounded `HandshakeResponse`, after which the server closes
  the connection and the Client requires EOF. A later task may add a post-handshake
  phase only under a newly accepted higher minor/major version; a peer negotiated to
  1.0 always retains this terminal-close behavior without changing published fields.
- The handshake has no durable effect, command ID, Project context, pagination,
  retry loop, audit write, Provider/Plugin behavior or idempotency record. Repeating a
  successful handshake creates only a new transport-scoped correlation ID.

### 5.1 Decode-depth enforcement

- Depth counts the root message as 1 and increments once for each known embedded
  message or map-entry payload selected by the committed descriptor. Strings, bytes,
  packed scalars and unknown length-delimited fields are opaque at their current
  depth. Deprecated protobuf group wire types 3/4 are non-canonical for this proto3
  contract and are rejected at every inspected level.
- The committed descriptor must have an acyclic reachable message graph. A build-time
  generator derives a compact field-kind table from that descriptor; the normal
  build includes the table with the generated Prost types and checks its descriptor
  digest. No descriptor parser or reflection dependency enters the runtime graph.
- Before `prost::Message::decode`, an allocation-free iterative preflight walks the
  bounded payload using the generated table, validates tags/varints/length arithmetic,
  descends only into known message fields, rejects groups, and fails with
  `VALIDATION_ERROR` before the next level would exceed the typed configured limit.
  Its explicit stack is bounded by 64 entries and the scan is O(frame bytes).
- The published TASK-003 graph has exact maximum schema depth 3 when the root counts
  as 1: `HandshakeResponse -> ErrorEnvelope -> SafeDetailsEntry`. `ClientHello` has
  depth 1 and `HandshakeResponse -> ServerHello` has depth 2. Descriptor regeneration
  fails if this graph gains a cycle or exceeds the accepted maximum of 64.
- `prost` keeps its default recursion guard as defense in depth, but that private
  100-level guard is not acceptance evidence. The `no-recursion-limit` feature is
  forbidden. `TEST-PROTO-001`, `TEST-FRAME-001` and `TEST-CONFIG-003` jointly prove
  descriptor/table drift, pre-decode ordering and tightening behavior.

## 6. Proposed bounded lifecycle

- A connection permit is acquired before a handshake task is spawned or payload is
  read. At capacity the accepted socket is closed; no unbounded pending task/queue or
  best-effort error allocation is created.
- `peer_cred()` is called before framing allocation. Missing evidence or
  `peer_uid != opened_library.owner_uid()` closes immediately.
- One absolute handshake deadline covers peer verification, header/payload read,
  protobuf decode/validation, correlation-ID generation, response write/flush and
  close. There is no internal retry.
- EOF before a complete header/payload is a terminal validation/protocol failure and
  creates no durable state. Client disconnect cancels and joins only that tracked
  handshake.
- Handshake tasks contain no blocking/unabortable work. Graceful shutdown linearizes
  by closing listener admission, cancelling every tracked handshake and joining it
  within the remaining absolute deadline. At the deadline, every unfinished Tokio
  task is explicitly aborted and its `JoinHandle` is still awaited before endpoint
  cleanup; panic, unexpected cancellation or join failure maps to `INTERNAL_ERROR`.
- Endpoint cleanup and store shutdown use finally-style error aggregation. A known
  socket is identity-revalidated before descriptor-relative unlink, and its runtime
  directory is synced after successful unlink. Any endpoint cleanup error is retained
  as `STORAGE_IO_ERROR`, but cannot skip `OpenedLibrary::shutdown`; the opened store
  is always shut down and the Library lock is released last. A store shutdown error
  is also retained. The first failure in the fixed task-join → endpoint-cleanup →
  store-shutdown order is the returned primary code; every later failure contributes
  only its stable redacted code to restricted diagnostics/metrics. No task or lock is
  detached merely because an earlier cleanup step failed.
- The TASK-003 client attempts one connection/handshake within the same deadline and
  performs no automatic retry. It returns bounded safe exit diagnostics.

## 7. Proposed opened-Library composition API

The store crate may expose exactly one opaque owner object and one copyable safe
identity view:

```rust
pub struct OpenedLibrary { /* private store, workers, authority and lock */ }
pub struct OpenedLibraryIdentity { /* private library UUID and owner UID */ }

impl OpenedLibrary {
    pub fn open_or_bootstrap(config: &StoreConfig) -> Result<Self, StoreError>;
    pub fn identity(&self) -> OpenedLibraryIdentity;
    pub fn shutdown(self) -> Result<(), StoreError>;
}

impl OpenedLibraryIdentity {
    pub fn owner_uid(self) -> u32;
    pub fn library_id_bytes(self) -> [u8; 16];
}
```

- Exact names may change during canonical review, but the visibility and authority
  properties may not: constructing the object is the only production open/bootstrap
  entry; holding it retains the TASK-004 worker/connection/lock lifecycle; shutdown
  consumes it and releases authority last.
- `open_or_bootstrap` first performs a read-only absent/safe-empty versus existing
  namespace classification. Absent/safe-empty uses the existing TASK-004 first-create
  path so clock and Library ID are sampled before root mutation. Existing state uses
  the existing lock-bound recovery path. A recovered `NeedsFreshBootstrap` state
  samples a new clock/ID once while retaining that lock, before recreating intent or
  staging; it never drops and reacquires authority. Every race is revalidated by the
  existing platform path/lock contract and fails closed.
- No `Connection`, SQL, row, migration, filesystem path, file descriptor, raw lock,
  `OpenedLibraryAuthority` or crate-private store command surface becomes public.
- `library_id_bytes` is used only to bind the runtime namespace marker; it is not a
  caller identity, command ID or permission token.
- Framing/proto crates do not depend on the store. Only the daemon composition root
  sees both `OpenedLibrary` and IPC constructors and passes the copyable identity view.

## 8. Proposed configuration and runtime endpoint contract

### 8.1 Immutable configuration

The daemon composition root owns one-time source capture and precedence. Store,
framing and platform crates accept only complete typed immutable DTOs and never read
CLI/environment/config sources.

TASK-003 consumes:

| Key | Default | Accepted behavior |
|---|---:|---|
| `MENGXIA_LIBRARY_ROOT` | none | required absolute canonical store root; existing TASK-004 validator remains authoritative |
| `MENGXIA_CLIENT_ENDPOINT` | `<validated platform-derived temp root>/mengxia-runtime-v1/client.sock` | absolute Unix path with exact basename `client.sock`; no NUL; encoded path must fit macOS `sun_path` including terminator |
| `MENGXIA_MAX_FRAME_BYTES` | 4 MiB | existing accepted range 64 KiB–16 MiB |
| `MENGXIA_MAX_DECODE_DEPTH` | 64 | existing tightening-only range 1–64; TASK-003 startup requires at least its descriptor-proven schema depth 3 |
| `MENGXIA_CLIENT_HANDSHAKE_TIMEOUT_MS` | 5000 | tightening-only 100–5000 ms |
| `MENGXIA_MAX_PENDING_HANDSHAKES` | 32 | accepted range 1–256 |

- CLI flag (non-secret) > named `MENGXIA_*` environment > Library config source >
  platform-derived or compiled default.
  Because TASK-003 defines no Library config file schema, its production resolver
  accepts an explicitly typed optional Library-config layer but does not open or
  invent such a file; adding the source reader belongs to its owning later task.
- The endpoint's platform-derived default is resolved exactly once by the composition
  root. On Unix, `std::env::temp_dir()` consults `TMPDIR`; therefore `TMPDIR` is an
  explicitly declared untrusted source for this default, not a compiled constant and
  not an alias for `MENGXIA_CLIENT_ENDPOINT`. If present, its exact `OsString` path is
  captured once and validated; an unsafe value fails closed without falling back to a
  different directory. The selected path is canonicalized and then subjected to the
  complete descriptor-relative owner/mode/ACL/local-filesystem contract below.
- `MENGXIA_LIBRARY_ROOT` has no compiled default and cannot be obtained from the
  Library config whose location it selects. CLI/environment are the only active
  TASK-003 sources for that key; this exception must be synchronized into canonical
  §16 rather than silently implemented.
- Missing, non-Unicode where the key's parser requires text, NUL, non-absolute,
  non-canonical, zero, signed, whitespace, overflow and out-of-range values fail
  before Library or runtime namespace mutation. No raw rejected value/path is logged.
- Decode-depth values 1 or 2 are globally well-formed tightening values but cannot
  represent TASK-003's exact depth-3 response schema; they disable/fail this sole
  daemon capability with `VALIDATION_ERROR` before opening/bootstrap of the Library
  or mutation of the runtime namespace. Values 3..=64 are accepted.
- Widening the frame/handshake maxima, decode-depth ceiling or timeout ceiling requires
  a recorded decision and boundary/overload regression evidence; these are security
  caps, not performance SLOs.

### 8.2 Runtime namespace authority

- The runtime directory is separate from the canonical Library root. The default is
  the validated platform-derived temporary root plus fixed child
  `mengxia-runtime-v1`; an override selects the socket path, whose parent is the final
  runtime directory and whose grandparent is its hosting directory.
- The hosting directory (the validated platform-derived temporary root by default)
  must already be a non-symlink local-filesystem directory owned by the Library owner
  UID, mode `0700`, empty ACL. An override whose hosting directory is shared,
  group/world accessible or not provably owner-only is rejected; this gives exclusive
  authority for creating or reopening the fixed final runtime-directory edge.
- The platform crate opens/revalidates the full prefix descriptor-relatively. The
  final runtime directory must be a non-symlink directory owned by the opened Library
  owner UID, mode exactly `0700`, empty ACL, and on the accepted local filesystem.
- An absent final runtime directory may be created owner-only. An existing directory
  without the exact canonical marker below is accepted only when empty or when its
  sole entry is one complete, typed, checksum-valid fixed staging marker matching the
  opened Library identity. Zero/partial/invalid staging, arbitrary content, unknown
  marker, symlink/type/owner/mode/ACL mismatch fails before unlink/link/bind and is
  retained unchanged.
- Fixed marker `.mengxia.runtime-owner-v1` is exactly 128 bytes, big-endian where
  numeric: bytes 0..16 = ASCII `MENGXIA-RUNTIME` plus one NUL; 16..18 = version 1;
  18..20 = total length 128; 20..36 = Library UUID bytes; 36..40 = owner UID as u32;
  40..96 = zero reserved bytes; 96..128 = SHA-256 of bytes 0..96. Its owner/mode/ACL
  are exact owner UID / `0600` / empty ACL; every other length, version, padding,
  checksum or typed UUID/UID mismatch fails closed.
- A partial canonical marker is impossible by contract. Its only staging name is the
  fixed `.mengxia.runtime-owner-v1.staging` inside the already validated final runtime
  directory; no random or sibling artifact is created. It is created descriptor-
  relatively with exclusive/no-follow semantics, exact owner/mode/ACL/link-count
  checks and bounded short-write handling. Its complete bytes and metadata are
  revalidated after `fsync` and immediately before it is hard-linked with no-replace
  semantics to the canonical marker. The directory is then `fsync`ed, the same-inode
  staging name is unlinked, and the directory is `fsync`ed again before any socket
  name is created.
- The marker-publication substate recognizes only: empty directory; safe staging only;
  canonical only; or canonical plus staging names that resolve to the same validated
  inode. Once a valid canonical marker exists, the separate socket substate below may
  additionally contain only the fixed `client.sock`. Staging-only and canonical-only
  require link count one; canonical-plus-staging requires the same inode with link
  count exactly two. A complete valid staging marker matching the opened Library may
  continue publication. A zero/partial staging file is never deleted, truncated,
  overwritten or recreated automatically: fixed name, regular type, owner, `0600`,
  empty ACL, link count one and length at most 128 are containment evidence only, not
  cleanup provenance. It is retained byte-for-byte and startup fails closed before
  link/unlink/bind. Canonical plus a distinct staging inode, any unexpected link count,
  an invalid/unsafe staging object or any unknown entry likewise fails closed without
  deletion. Completed link/unlink/directory-sync prefixes remain recognized on the
  next same-OS start; a crash or write error that leaves zero/partial staging is a
  preserved operator-visible fail-closed state, not an automatic recovery claim.
- Fixed socket basename is `client.sock`. After bind, it is set to owner-only mode
  and the final parent/socket edge, UID, mode and type are revalidated before accept.
  If bind succeeds but mode-setting, revalidation or listener publication fails, the
  daemon revalidates and removes only that exact just-bound inode, syncs the directory,
  records any cleanup failure and still runs the §6 store-shutdown path. It never
  returns success or begins admission with a partially published endpoint.
- If `client.sock` already exists, the daemon validates marker/directory/socket first
  and performs one bounded connection probe. A successful connection or ambiguous
  error returns `STORAGE_CONFIGURATION_ERROR` without deletion. Only a proven matching
  marker plus a socket whose probe returns exact `ECONNREFUSED` may be treated as
  stale; identity is revalidated immediately before descriptor-relative unlink. Any
  unproven same-name object fails closed. `BACKPRESSURE` is reserved for admitted IPC
  capacity, not endpoint collision.
- Normal shutdown closes listener/admission, joins all handshakes, unlinks the exact
  revalidated socket and syncs the runtime directory under §6's error-aggregation
  rule. The marker/directory may remain for the same Library. SIGKILL recovery is
  same-OS only; no power-loss claim is made.
- The CLI captures its effective UID once through the accepted platform boundary and
  requires the hosting/final directories and socket to be owned by that captured UID
  with the exact type/mode/ACL/local-filesystem rules above. After
  connect and before writing any frame, it obtains the server peer credential through
  the same safe platform API and requires that peer UID to equal the captured Client
  eUID. This is same-owner endpoint authentication, not inference of the durable
  Library owner; mismatch/missing evidence returns `AUTHENTICATION_ERROR` without a
  request frame. eUID 0 follows the same equality checks but receives no containment
  claim; root and same-owner-account compromise remain outside ADR-0004's boundary.
  Other UID access is denied by directory/socket policy plus mutual peer checks.
- Production never weakens those permissions to test server-side mismatch handling.
  `TEST-IPC-MACOS-001` uses the private §3 CI-only listener fixture for that branch and
  `TEST-ARCH-003` proves the fixture, permissive bind path and every associated switch
  are absent from non-test artifacts.

### 8.3 Exact TASK-003 CLI/daemon contract

TASK-003 publishes exactly two effectful command forms and two static help forms:

```text
mengxiad serve [--library-root PATH] [--client-endpoint PATH]
  [--max-frame-bytes ASCII_U64] [--max-decode-depth ASCII_U32]
  [--client-handshake-timeout-ms ASCII_U64]
  [--max-pending-handshakes ASCII_U32]
mengxiad --help

mengxia handshake [--client-endpoint PATH] [--max-frame-bytes ASCII_U64]
  [--max-decode-depth ASCII_U32]
  [--client-handshake-timeout-ms ASCII_U64]
mengxia --help
```

- Command and long-option spellings above are exact. Every value is the following
  separate argv item; short options, `--name=value`, `--`, positionals, repeated
  options, missing values, unknown options/subcommands and effectful no-argument
  invocation are rejected. Numeric argv values are non-empty ASCII decimal digits
  only, without sign or whitespace, and pass the same typed ranges as §8.1. Path argv
  values remain `OsString` until the platform/store validator consumes them; they are
  never lossily converted for parsing, display or diagnostics.
- Each listed daemon flag is the CLI layer for the same-named §8.1 `MENGXIA_*` key.
  `mengxia handshake` owns CLI layers only for endpoint, frame, decode-depth and
  handshake timeout. It does not read or require `MENGXIA_LIBRARY_ROOT`, infer a
  Library owner or accept an actor/UID/role/request-ID argument. An absent flag falls
  through to the exact environment/optional typed Library-config/default rules in
  §8.1; an invalid higher-precedence value fails rather than falling through.
- `mengxia handshake` generates exactly one request ID with the accepted fallible
  UUIDv7 generator after complete CLI/config and protected-endpoint validation but
  before connect. It never accepts caller-supplied request, correlation, actor or
  authority fields. Generation failure returns `ID_GENERATION_UNAVAILABLE` and does
  not connect.
- Successful `mengxia handshake` writes exactly one ASCII line to stdout and nothing
  to stderr, then exits 0:

  ```text
  MENGXIA_HANDSHAKE_OK protocol=1.0 request_id=<canonical UUIDv7> correlation_id=<canonical UUIDv7>
  ```

  The two IDs must equal the validated response fields. Extra response bytes,
  non-empty 1.0 `safe_details` or missing terminal EOF turns the command into failure;
  no success prefix may already have been written.
- Every command failure writes nothing to stdout and exactly one bounded ASCII line
  to stderr, using only the accepted registry code, then exits as specified below:

  ```text
  MENGXIA_ERROR code=<ERROR_CODE>
  ```

  Raw argv/environment values, paths, frames, UID/GID/PID, OS diagnostics and peer
  response text are never interpolated. Argv/source/typed-value validation that fails
  before Library or endpoint access exits 2; every post-validation startup, transport,
  protocol, server response or orderly-shutdown failure exits 1. No distinct exit
  value exposes peer identity or endpoint liveness.
- A server-side pre-response close intentionally carries no error oracle. The server
  records its own `AUTHENTICATION_ERROR` or `BACKPRESSURE`, while the client maps
  connect/write/flush/read/reset or EOF before a complete response to generic
  `IPC_TRANSPORT_ERROR`; it must not infer or print the server's unobserved reason. A
  received valid error envelope uses its exact code. A complete bounded response whose
  framing/protobuf/content/trailing bytes are invalid, including a `ServerHello` outside
  the offered range, is `VALIDATION_ERROR`; the server uses
  `PROTOCOL_VERSION_UNSUPPORTED` only when a valid `ClientHello` has no common version.
  Expiry of the one absolute client deadline is `DEADLINE_EXCEEDED`. The same observed
  failure cannot be remapped by its underlying OS errno, partial byte count or server
  liveness.
- `mengxiad serve` is a foreground process: it never daemonizes/forks, creates no PID
  file, writes no readiness or protocol bytes to stdout, and reaches readiness only
  through the validated socket becoming connectable. After argv/config validation it
  opens the Library before mutating the runtime namespace. The first SIGINT or SIGTERM
  starts exactly §6's admission-close, joined-handshake, endpoint-cleanup and consuming
  store-shutdown sequence. Additional graceful signals do not bypass cleanup; SIGKILL
  remains the already specified same-OS recovery case. A clean signal shutdown writes
  nothing and exits 0. A startup/runtime/shutdown error emits the one safe error line
  and exits under the mapping above; only one primary error is public.
- `--help` is side-effect free: it performs no environment/config capture, filesystem
  access, ID/clock generation, socket operation or store open, prints the corresponding
  static grammar above to stdout, writes nothing to stderr and exits 0. TASK-003 adds
  no version command, shell completion, JSON mode, daemon-management command or
  product operation. Later additions require their owning task's accepted public
  contract and cannot alter the TASK-003 handshake behavior.

## 9. Proposed dependencies and reproducible code generation

Exact proposed pins, all default features disabled:

```toml
prost = { version = "=0.14.4", default-features = false, features = ["derive", "std"] }
prost-types = { version = "=0.14.4", default-features = false, features = ["std"] } # build/descriptor only
prost-build = { version = "=0.14.4", default-features = false } # build only
tokio = { version = "=1.53.1", default-features = false, features = ["io-util", "macros", "net", "rt-multi-thread", "signal", "sync", "time"] }
```

- `prost` 0.14.4 is the only runtime protobuf dependency. `prost-types` is not exposed
  in the public protocol because TASK-003 has no well-known-type field.
- Normal and `--offline --locked` Cargo builds use the committed descriptor with
  `prost-build::compile_fds`; they never execute or discover ambient `protoc`, read
  `PROTOC`/`PROTOC_INCLUDE`, download a compiler or use network access.
- Canonical `.proto`, its deterministic `FileDescriptorSet` and a small provenance
  manifest containing the exact `.proto` SHA-256, descriptor SHA-256, compiler
  release/artifact SHA-256 and generator version are committed. The normal build
  checks both file digests before `compile_fds`, so a local schema edit cannot compile
  against stale generated types. Generated Rust is an OUT_DIR artifact, not a second
  manually edited source of truth.
- Explicit regeneration uses only official
  `protoc-35.1-osx-aarch_64.zip` after exact SHA-256 verification. It invokes the
  absolute extracted compiler with fixed include/input/output arguments in a temporary
  directory and rejects `PATH`, `PROTOC`, `PROTOC_INCLUDE` and extra include/plugin
  overrides. It does not install or mutate the developer's global toolchain.
- `TEST-PROTO-001` compares regenerated descriptor bytes and descriptor-decoded
  package/message/field/reserved metadata. Network/tool unavailability is
  `UNVERIFIABLE`, never PASS. Default local edit/build/test remains usable without
  regeneration; reviewed formal CI supplies fresh compiler evidence.
- No `tonic`, HTTP, TCP, Serde, SQLite dependency in proto/framing, CAS, `tokio-util`,
  `nix`, direct `libc`, shell execution or unsafe MengXia IPC code is introduced.
- `TEST-SUPPLY-003` verifies exact features, lockfile, licenses, advisories, compiler
  digest and absence of unwanted defaults/duplicates/ambient compiler inputs.

## 10. Authority, errors and security verification

- `PrincipalContext` is private server state created only after `peer_cred()` succeeds
  and the UID equals `OpenedLibraryIdentity::owner_uid()`.
- Ordinary Client is the only constructible production authority. The production
  listener and dispatch registry contain no Admin route, session, constructor or
  handler. A private `#[cfg(test)]` pure authorization seam may receive a synthetic
  Admin-domain label plus a panic-on-call continuation solely to prove that it returns
  `ADMIN_AUTH_UNAVAILABLE` before invoking that continuation; it is absent from the
  non-test symbol/API graph and cannot submit a command, store work, audit or intent.
- TASK-003 has no tenant or Project-scoped product command and makes no tenant-isolation
  claim. The authenticated principal represents the one Library-owner OS trust domain.
- Exact error mapping:
  - missing/malformed/out-of-range daemon config or a decode-depth value below the
    descriptor-proven TASK-003 minimum → `VALIDATION_ERROR` before namespace access;
  - unsafe/missing peer evidence → `AUTHENTICATION_ERROR`, close without response;
  - invalid frame/protobuf/hello/ID/range → `VALIDATION_ERROR`, at most one safe frame;
  - incompatible version → `PROTOCOL_VERSION_UNSUPPORTED`;
  - handshake capacity full → `BACKPRESSURE` and immediate close;
  - unsafe/live/ambiguous runtime endpoint namespace →
    `STORAGE_CONFIGURATION_ERROR` before listener admission;
  - zero/partial/invalid or otherwise unproven marker staging →
    `STORAGE_CONFIGURATION_ERROR`, preserve its exact bytes/inode and perform no
    unlink/link/bind;
  - bind/mode/link/unlink/sync and other runtime-namespace syscall I/O failure →
    `STORAGE_IO_ERROR`; startup remains failed and shutdown still releases the opened
    Library authority under §6 even when endpoint cleanup also fails;
  - Client connect/write/flush/read/reset or EOF before one complete response →
    `IPC_TRANSPORT_ERROR`; do not expose errno, distinguish absent/refused/live or
    infer the server's unobserved authentication/backpressure reason;
  - server socket read errors other than the explicit malformed/truncated EOF case,
    and response write/flush/close errors → `IPC_TRANSPORT_ERROR` in restricted
    diagnostics/metrics; if no complete error frame was already sent, close without
    attempting a second response;
  - deadline → proposed new `DEADLINE_EXCEEDED`; never classify an expected bounded
    transport timeout as storage/provider failure or an internal invariant bug;
  - correlation-ID failure → `ID_GENERATION_UNAVAILABLE`;
  - panic/join/invariant failure → `INTERNAL_ERROR` and listener mutation admission
    stops until orderly shutdown.
- Proposed registry rows, each preserving TASK-002's strict parser and
  `Display == as_str()` contract:
  - `IPC_TRANSPORT_ERROR`; source = local IPC connect/write/flush/read/close transport;
    retryability = caller may start one new request only within its
    own bounded budget and only after revalidating configuration/endpoint, server never
    auto-retries; API exposure = static safe transport guidance with no endpoint/errno/
    peer detail; log level = `INFO/WARN`; metric = `ipc_transport_errors_total`; Rust
    variant/parser/display = `IpcTransportError` / `IPC_TRANSPORT_ERROR` /
    `IPC_TRANSPORT_ERROR`.
  - `PROTOCOL_VERSION_UNSUPPORTED`; source = authenticated local IPC version
    negotiation with no common supported version; retryability = no until compatible
    software/configuration exists; API exposure = static safe compatibility guidance,
    no peer-supplied version echo; log level = `INFO`; metric =
    `protocol_version_unsupported_total`; Rust variant/parser/display =
    `ProtocolVersionUnsupported` / `PROTOCOL_VERSION_UNSUPPORTED` /
    `PROTOCOL_VERSION_UNSUPPORTED`.
  - `DEADLINE_EXCEEDED`; source = local IPC absolute deadline; retryability =
    caller may start one new request only within its own bounded budget, server never
    auto-retries; API exposure = static safe timeout guidance; log level = `INFO/WARN`;
    metric = `deadline_exceeded_total`; Rust variant/parser/display =
    `DeadlineExceeded` / `DEADLINE_EXCEEDED` / `DEADLINE_EXCEEDED`.
- These three stable codes require explicit acceptance and canonical §14.1 plus
  TASK-002 exhaustive round-trip/redaction fixture synchronization. They do not change
  an existing code's retryability or overload Provider/Plugin/storage semantics.
- The existing `STORAGE_CONFIGURATION_ERROR` source text must be synchronized to
  include an unsafe/ambiguous local runtime filesystem namespace; its retryability,
  redaction, log level and metric remain unchanged. No new endpoint-specific code is
  needed for that startup condition.

The error mapping exposes a canonical conflict and MUST be resolved in the same
Decision/Specification synchronization rather than treated as an editorial addition:

```text
CONFLICT:
Source A: Specification v1.1.14 limits STORAGE_IO_ERROR to filesystem/backend failures and UNSUPPORTED_CAPABILITY to declared Provider/Plugin capability contracts, and defines no stable local IPC transport, version-negotiation or deadline code.
Source B: TASK-003 must return one stable redacted code for Client socket failure, incompatible protocol version and bounded deadline without misclassifying them as storage, Provider/Plugin or INTERNAL_ERROR.
Recommended canonical decision: preserve every existing error-code meaning and add exactly IPC_TRANSPORT_ERROR, PROTOCOL_VERSION_UNSUPPORTED and DEADLINE_EXCEEDED with the rows above.
Reason: distinct transport/version/deadline classifications make retry and operator behavior deterministic without exposing endpoint liveness, errno or peer-controlled values.
Impact: TASK-003 may update only mengxia-types, retained TASK-002 exhaustive fixtures and canonical §14.1 for these three variants; later task scope and existing code semantics do not change.
```

Acceptance of this conflict resolution MUST be recorded in `DECISIONS.md` before the
start record is activated. Specification and Decisions must each contain this exact
machine-checkable block once:

```text
TASK003_ERROR_TAXONOMY_CONFLICT: ACCEPTED
TASK003_ERROR_CODES_ADDED: IPC_TRANSPORT_ERROR; PROTOCOL_VERSION_UNSUPPORTED; DEADLINE_EXCEEDED
TASK003_STORAGE_IO_SOURCE_PRESERVED: filesystem/backend
TASK003_UNSUPPORTED_CAPABILITY_SOURCE_PRESERVED: declared Provider/Plugin capability contract
```
- No secret exists in TASK-003. Logs may contain safe error code, bounded operation
  name, correlation ID after creation and duration; never raw frame, path, UID/GID/PID,
  environment, OS error text or rejected input.

## 11. Proposed stable registry and start-record template

### Acceptance IDs

`AC-028` and `AC-029` remain canonical cross-task acceptance behaviors with exactly
the definitions already published in Specification §19.3. This proposal does not
redefine them or claim that TASK-003 can complete their audit/product-operation
branches: TASK-003 forbids audit writes and product/Admin operations. Its actor-spoof
and disabled-Admin evidence contributes to those later V1 obligations, but TASK-003
does not record either ID as `PASS`. Canonical synchronization removes AC-028/AC-029
from the TASK-003 row/start/completion set without changing their definitions and adds
the following exact contributor/terminal-owner mapping to Specification, Decisions
and Plan:

- `AC-028`: TASK-003 contributes reserved actor-wire and channel-derived principal
  evidence; TASK-007 contributes `CommandRecord` binding; TASK-013 owns the first real
  `SecurityAuditEvent` attribution and is the sole task permitted to record terminal
  `AC-028: PASS` after the complete IPC → principal → command → audit chain passes.
- `AC-029`: TASK-003 contributes production Admin-route absence; TASK-013 owns the
  first privileged-dispatch authorization boundary and contributes ordinary-Client
  denial for Plugin grant, audit export and manual/destructive Library migration
  administration; TASK-016 contributes Credential denial and TASK-022 Purge denial.
  TASK-023 is the sole terminal owner and may record `AC-029: PASS` only after the
  complete matrix returns exactly `AUTHORIZATION_DENIED` or
  `ADMIN_AUTH_UNAVAILABLE` with no privileged intent or state. An operation that is
  intentionally unexposed still requires an accepted canonical dispatch contract for
  the attempted ordinary-Client request; absence, `UNIMPLEMENTED` or
  `UNSUPPORTED_CAPABILITY` is not `AC-029` PASS.

This mapping exposes a pre-existing canonical conflict and MUST NOT be synchronized as
an editorial change:

```text
CONFLICT:
Source A: Specification v1.1.14 says Option A changes only TASK-004/TASK-003 ordering and does not change later Task scope or acceptance.
Source B: canonical AC-028/AC-029 span CommandRecord, audit and named privileged operations that TASK-003 forbids, so their current TASK-003/TASK-013/TASK-016 completion references cannot produce honest terminal PASS evidence.
Recommended canonical decision: preserve every stable Task and AC definition, but correct completion ownership and the minimum dependency/scope edges exactly as listed below.
Reason: contributor evidence must not block an earlier task or permit premature PASS, and every canonical branch needs an implementation owner before the release gate verifies it.
Impact: TASK-013 adds TASK-007 plus the narrow privileged-dispatch denial boundary and terminal AC-028; TASK-016 drops premature AC-029; TASK-023 becomes terminal AC-029 owner; no task receives implementation authority from this correction.
```

Acceptance of this conflict resolution MUST be recorded in `DECISIONS.md` in the same
canonical synchronization that changes Specification and Plan.

These are traceability assignments, not authority to start any named later task. The
canonical Specification, Decisions and Plan record the accepted conflict resolution
once each with this exact block:

```text
TASK003_AC_OWNERSHIP_CONFLICT: ACCEPTED
TASK003_AC_028_CONTRIBUTORS: TASK-003; TASK-007
TASK003_AC_028_TERMINAL_OWNER: TASK-013
TASK003_AC_029_CONTRIBUTORS: TASK-003; TASK-013; TASK-016; TASK-022
TASK003_AC_029_TASK013_BRANCHES: PLUGIN_GRANT; AUDIT_EXPORT; MANUAL_MIGRATION_ADMIN
TASK003_AC_029_TASK016_BRANCHES: CREDENTIAL
TASK003_AC_029_TASK022_BRANCHES: PURGE
TASK003_AC_029_TERMINAL_OWNER: TASK-023
```

To preserve §0.5's rule that every AC referenced by a task must be PASS before that
task is `DONE`, canonical synchronization also makes these exact Plan changes:
TASK-013 adds TASK-007 as a dependency, replaces its premature AC-029 completion
reference with terminal AC-028, and adds only the privileged-dispatch denial boundary
needed for Plugin grant, audit export and manual/destructive migration administration;
TASK-016 removes AC-029 from its completion set; and TASK-023 adds terminal AC-029.
TASK-007, TASK-013, TASK-016 and TASK-022 contributor relationships remain only in the
machine block until the terminal owner records PASS; they are not permission for an
earlier task to claim partial AC completion.

The exact TASK-003 acceptance set is therefore:

- `AC-060`: frame codec rejects zero, malformed, truncated and cap+1 input before
  payload allocation; cap-1 and cap round-trip exactly.
- `AC-061`: exact proto package/messages/fields/reservations and version negotiation
  are deterministic; UUIDv7 values are canonical and wire values never grant authority.
- `AC-062`: real macOS UDS handshake constructs ordinary Client context only after
  peer UID matches the durable Library owner; missing/mismatched evidence fails first.
- `AC-063`: CLI reaches only the handshake daemon UDS; binaries and dependency graph
  expose no SQLite/CAS/TCP/Admin/product-operation bypass.
- `AC-064`: endpoint ownership/stale recovery, handshake deadline/capacity,
  disconnect and daemon shutdown remain bounded, joined and free of durable effects.

### Test IDs

- `TEST-PROTO-001`: descriptor/package/field/reservation/golden regeneration drift.
- `TEST-FRAME-001`: framing property/fuzz-style matrix, allocation-order evidence and
  descriptor-derived pre-decode depth/group/malformed-wire matrix.
- `TEST-HANDSHAKE-001`: version/ID/order/error/empty-safe-details/deadline/disconnect
  behavior, including exact `PROTOCOL_VERSION_UNSUPPORTED`, `IPC_TRANSPORT_ERROR`,
  `DEADLINE_EXCEEDED` separation and terminal close for negotiated 1.0.
- `TEST-IPC-MACOS-001`: real same-UID handshake; real second-UID OS denial against the
  production owner-only namespace; and, in the same reviewed formal run, real
  second-UID pre-frame rejection through the private `#[cfg(test)]` reachability-only
  fixture using the exact production peer-credential/handshake function.
- `TEST-ENDPOINT-003`: runtime marker/staging/socket ownership, collision, stale/live,
  path, every bind/mode/link/unlink/sync failure, SIGKILL recovery and shutdown-order/
  lock-release matrix; zero/partial/unproven staging is retained unchanged and fails
  before unlink/link/bind rather than being treated as cleanup authority.
- `TEST-CONFIG-003`: source precedence/absence/non-Unicode/NUL/path/cap boundary and
  decode-depth 1/2 rejection plus 3/64 acceptance before any namespace mutation.
- `TEST-AUTH-001`: actor-tag spoof, principal injection, production Admin-registry
  absence and the private `#[cfg(test)]` fail-before-continuation seam.
- `TEST-CLI-001`: exact §8.3 subprocess grammar; side-effect-free help/parse failures;
  source precedence; generated request ID; success/error stdout/stderr and exit codes;
  response-less close classification; no raw-value disclosure; foreground daemon
  SIGINT/SIGTERM joined shutdown and safe exits.
- `TEST-ARCH-003`: dependency/API/no-DB/CAS/TCP/Admin/unsafe/later-scope checks.
- `TEST-SUPPLY-003`: exact dependency/compiler/descriptor/license/advisory policy.
- `TEST-DOC-003`: registry/start/completion/current-state and negative traceability.

### Accepted start record — active

```text
TASK: TASK-003
STATUS: IN_PROGRESS
DEPENDENCIES: TASK-002; TASK-004; BASE-007; BASE-008; BASE-012; BASE-013;
  BASE-014; BASE-016; BASE-017; DEC-007; DEC-012; DEC-016; DEC-017;
  DEC-019; DEC-021; DEC-022; ADR-0001; ADR-0004; ADR-0005
REQUIREMENTS: FUNC-001; API-001; API-002; API-003; API-008; API-009;
  API-010; SEC-005; SEC-013; SEC-014; SEC-017; SEC-020; SEC-021;
  REL-001; REL-006; CFG-001; CFG-003
ACCEPTANCE: AC-060; AC-061; AC-062; AC-063; AC-064
TESTS: TEST-PROTO-001; TEST-FRAME-001; TEST-HANDSHAKE-001;
  TEST-IPC-MACOS-001; TEST-ENDPOINT-003; TEST-CONFIG-003; TEST-AUTH-001;
  TEST-CLI-001; TEST-ARCH-003; TEST-SUPPLY-003; TEST-DOC-003
AUTHORIZED: exact §4 scope only
FORBIDDEN: exact §4 forbidden list
```

All blocker, error-registry, AC/TEST, traceability and retained-baseline preconditions
were satisfied during canonical synchronization. The exact record above is active in
the canonical Plan and authorizes TASK-003 only; it is not a template for any later
task.

Before TASK-003 becomes `DONE`, the repository MUST contain three executable regular
non-symlink files: `scripts/verify-task-003.sh`,
`scripts/verify-task-003-formal-second-uid.sh` and
`scripts/run-task-003-second-uid.sh`. The first is the unprivileged aggregate developer
command and maps the ten non-privileged TEST IDs to deterministic repository checks
while retaining the complete TASK-001/TASK-002/TASK-004 baseline. It may run the
same-UID portion of `TEST-IPC-MACOS-001`, but MUST NOT emit that ID as PASS without the
real second-UID branches. The third script is a private formal-test runner: it performs
§3's exact bounded account-name/UID selection, installs cleanup before mutation,
creates/revalidates the disposable account, runs both real-second-UID cases, deletes
and proves absence of the account, and exits nonzero for any test or cleanup failure.
It prints no TEST PASS result itself. The formal verification script first invokes the
unprivileged aggregate and then owns exactly this mapping:

```text
task003_run TEST-IPC-MACOS-001 -- ./scripts/run-task-003-second-uid.sh
```

Consequently the formal wrapper emits PASS only after the privileged runner has
completed both evidence branches and cleanup. Each verification script prints one
exact `<TEST-ID>: PASS` result for every ID it owns and exits nonzero on a missing/
non-PASS owned result.

Mappings are executable statements, not comments or free text. Each owned ID appears
exactly once using this shell-token grammar with a non-empty direct argv after `--`;
the first argv must be `cargo` or an executable checked-in `./scripts/...` path:

```text
task003_run TEST-PROTO-001 -- cargo test <exact repository target arguments>
```

The developer script has exactly ten `task003_run` statements for its ten owned IDs.
The formal script contains exactly the one statement above for
`TEST-IPC-MACOS-001` and one standalone exact `./scripts/verify-task-003.sh` invocation
before it. IDs in comments,
duplicate statements, a missing `--`/argv, shell `eval`/`sh -c`, a command that does
not execute, a nonzero command converted to PASS, or a result emitted without its
mapped command completing successfully are invalid. `TEST-DOC-003` supplies negative
fixtures for each case, including a comment-only fake map and a failing mapped command.

TASK-003 completion evidence must use exactly these AC-to-TEST mappings:

```text
`AC-060`: `PASS`; EVIDENCE: TEST-FRAME-001
`AC-061`: `PASS`; EVIDENCE: TEST-PROTO-001+TEST-HANDSHAKE-001
`AC-062`: `PASS`; EVIDENCE: TEST-IPC-MACOS-001
`AC-063`: `PASS`; EVIDENCE: TEST-AUTH-001+TEST-CLI-001+TEST-ARCH-003
`AC-064`: `PASS`; EVIDENCE: TEST-HANDSHAKE-001+TEST-ENDPOINT-003+TEST-CONFIG-003
```

Every TEST evidence line must use its own exact stable fragment, for example:

```text
`TEST-PROTO-001`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-PROTO-001
```

The same form applies to the other nine unprivileged TEST IDs. The sole privileged
mapping is exactly:

```text
`TEST-IPC-MACOS-001`: `PASS`; EVIDENCE: scripts/verify-task-003-formal-second-uid.sh#TEST-IPC-MACOS-001
```

Arbitrary prose, a bare `pass`, a fixture label or a reference to another TEST
fragment is invalid. Both checked scripts, every mapped command and every referenced
target must exist in the candidate commit before `DONE`.

It must also record the reviewed real-second-UID formal CI evidence with exact
repository/workflow/job/runner binding as:

```text
FORMAL_SECOND_UID_CI_REPOSITORY: XiaTian-X/MengXia
FORMAL_SECOND_UID_CI_WORKFLOW: .github/workflows/ci.yml
FORMAL_SECOND_UID_CI_JOB: task-003-second-uid
FORMAL_SECOND_UID_CI_RUNNER: macos-26
FORMAL_SECOND_UID_CI_COMMIT: <40 lowercase hexadecimal candidate commit>
FORMAL_SECOND_UID_CI_RUN: <positive decimal GitHub Actions run ID>
FORMAL_SECOND_UID_CI_RESULT: PASS
```

The reviewed run must be from that repository/workflow/job at the recorded commit and
must prove both the production-path OS denial and private-fixture pre-frame rejection.
Prose mentions, duplicate rows, `SKIP`, `PARTIAL`, `UNVERIFIABLE`, a deterministic
synthetic UID mismatch or a run for another commit is not completion evidence.

Canonical synchronization must also add exactly one machine-checkable acceptance
record to each of Specification, Decisions, Review, Plan, Intake and `AGENTS.md`:

```text
TASK003_CANONICAL_GATE: ACCEPTED
TASK003_SPECIFICATION_VERSION: <current accepted Specification version>
TASK003_LIFECYCLE: IN_PROGRESS
TASK003_PROPOSAL: docs/proposals/TASK-003-GATE-PROPOSAL.md
```

The version and lifecycle values must agree with the proposal status and Plan row.
The record changes to `TASK003_LIFECYCLE: DONE` only in the same canonical change
that adds the completion record. A matching Specification version alone is not
evidence that Decision/Review/current-state documents incorporated this contract.

## 12. Accepted decisions and canonical synchronization

The user accepted the following decisions for TASK-003 on 2026-08-25:

1. Accept or revise the exact wire schema and handshake-only close semantics in §5.
2. Accept or revise 5,000 ms / 32-handshake finite lifecycle caps in §§6/8.
3. Accept or revise the narrow public opened-Library identity/lifetime seam in §7.
4. Accept or revise the separate marked runtime namespace/stale-socket protocol in §8.
5. Accept or revise Protocol Buffers v35.1 plus descriptor-first offline build policy
   in §9.
6. Accept or revise §10's explicit error-taxonomy `CONFLICT` resolution and exact
   `IPC_TRANSPORT_ERROR`, `PROTOCOL_VERSION_UNSUPPORTED` and `DEADLINE_EXCEEDED` rows.
7. Accept or revise the descriptor-derived pre-decode depth contract in §5.1 and
   exact `MENGXIA_MAX_DECODE_DEPTH` startup behavior in §8.1.
8. Accept the explicit `TMPDIR`-derived default-source classification and fail-closed
   validation rule in §8.1.
9. Accept the zero/partial/unproven marker-staging preservation rule and limited
   same-OS recovery claim in §8.2.
10. Accept the explicit §11 `CONFLICT` resolution, exact five-AC/eleven-TEST TASK-003
   registry, AC-028/AC-029 contributor/terminal-owner map without redefinition or
   premature PASS, `REL-001`, and the exact §4 scope.
11. Accept or revise the exact two-command CLI/daemon grammar, generated request ID,
    output/exit mapping and signal lifecycle in §8.3.
12. Accept the exact formal `macos-26` disposable non-root account lifecycle in §3;
    it remains completion-only CI evidence and never becomes a local developer gate.

All twelve decisions are confirmed by the synchronized canonical documents and exact
Plan start record. `TASK-003` is `IN_PROGRESS`; this acceptance does not authorize
TASK-005 or any later implementation task.
