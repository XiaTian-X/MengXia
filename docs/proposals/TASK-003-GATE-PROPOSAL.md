# TASK-003 start-gate proposal

> Status: **OPTION A ACCEPTED / TASK ORDER INCORPORATED; WIRE GATE STILL DRAFT**
>
> Date: 2026-08-21
>
> Scope: documentation-only readiness analysis for `TASK-003`; this file does not
> authorize production implementation and does not change a canonical decision.

## 1. Outcome

`TASK-003` is not ready to become `IN_PROGRESS` yet. The user accepted Option A on
2026-08-21, so TASK-004 now precedes TASK-003 and supplies durable Library owner/lock
context. Its product/security direction
is accepted, but the current task order cannot satisfy the accepted peer-authority
contract without either pulling `TASK-004` persistence into TASK-003 or weakening
ADR-0004. Several wire/build/lifecycle details required by `API-010` are also absent.

The accepted minimum safe resolution is **Option A** below: run the TASK-004 start gate and
implementation before TASK-003, then bind IPC authentication to the durable Library
owner and Library lock supplied by TASK-004. This changes execution order only; it
does not change architecture or make TASK-003 depend on SQLite directly.

## 2. Evidence read

- `AGENTS.md`: TASK-003 is only the next candidate and needs its own stable registry.
- Specification v1.1.7 §0.5: stable AC/TEST IDs and a task-start record are mandatory
  before implementation.
- Specification v1.1.7 §10.1: `PrincipalContext` is server-derived and never present
  in a request body.
- Specification v1.1.7 §12.3 and ADR-0004: the peer effective UID must equal the
  **recorded Library owner UID**; failure to establish that evidence fails closed.
- Specification v1.1.7 TASK-003: protected daemon/CLI handshake, framed proto3,
  version negotiation, request/correlation IDs, selected-platform peer verification,
  ordinary Client policy and disabled Admin.
- Specification v1.1.7 TASK-004 and ADR-0004: TASK-004 owns first-create bootstrap,
  durable Library owner recording, Library lock and startup lifecycle.
- ADR-0005: Core Protobuf frames default to 4 MiB, accepted range 64 KiB–16 MiB.
- Repository at commit `1228f0a`: TASK-002 is committed and the worktree was clean;
  IPC/proto/framing crates and binaries remain empty skeletons.
- Host diagnostic: no `protoc` executable is present.
- Official crate evidence checked 2026-08-21: `prost`, `prost-types` and `prost-build`
  latest are 0.14.4; `prost-build` requires `protoc` unless generation is driven from
  a supplied descriptor; Tokio latest is 1.53.1 and its safe
  `tokio::net::UnixStream::peer_cred()` implementation uses `getpeereid` on macOS.

## 3. Stable blockers

### `TASK003-BLOCKER-001` — durable owner authority is unavailable

- Category: `ARCHITECTURE`
- Severity: `CRITICAL`
- Scope: `TASK_SPECIFIC`
- Owner: `USER_DECISION_REQUIRED` / canonical plan revision
- Problem: TASK-003 must authenticate against the recorded Library owner, but
  TASK-004 owns the first durable creation of that owner and currently comes later.
- Why blocking: using daemon eUID, an environment variable or a request field as a
  temporary owner would contradict ADR-0004; adding owner persistence to TASK-003
  would cross the store/migration boundary.
- Minimum action: accept Option A or Option B in §4.
- Verification: the accepted order/scope must identify exactly who creates and who
  consumes `LibraryOwnerUid`; no IPC code may persist or infer it.

### `TASK003-BLOCKER-002` — public wire/framing contract is incomplete

- Category: `SPECIFICATION`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `WORK_SHOULD_FIX`
- Problem: “framed proto3” does not specify the length-prefix encoding, zero-frame
  behavior, cap inclusivity, allocation order, handshake message fields, version
  negotiation or failure/close semantics.
- Minimum action: accept the contract in §5 before code or publish a replacement.
- Verification: descriptor/golden/property tests and cap−1/cap/cap+1 checks.

### `TASK003-BLOCKER-003` — bounded lifecycle contract is incomplete

- Category: `SECURITY`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`
- Problem: only the frame-size cap is accepted. A listener/handshake also needs a
  finite handshake deadline, bounded simultaneous handshakes, disconnect behavior
  and shutdown ownership. Unbounded spawned tasks would violate `REL-006`,
  `SEC-021` and the no-detached-task rule.
- Minimum action: accept the conservative caps in §6 or record different finite caps.
- Verification: timeout, saturation, cancellation and shutdown tests prove that no
  connection task detaches and no state/privileged intent is written.

### `TASK003-BLOCKER-004` — proto code-generation policy is absent

- Category: `DEPENDENCY`
- Severity: `HIGH`
- Scope: `TASK_SPECIFIC`
- Owner: `USER_DECISION_REQUIRED` / `WORK_SHOULD_FIX`
- Problem: the host has no `protoc`; relying on ambient PATH would make builds
  environment-dependent, while silently vendoring/downloading a compiler adds a
  supply-chain decision.
- Minimum action: accept the descriptor-based offline build policy in §7 or choose
  and pin another compiler policy.
- Verification: locked offline workspace build succeeds without ambient `protoc`;
  an explicit regeneration check rejects schema/descriptor/generated-code drift.

### `TASK003-BLOCKER-005` — AC/TEST registry and Admin-negative seam are absent

- Category: `SPECIFICATION`
- Severity: `CRITICAL`
- Scope: `TASK_SPECIFIC`
- Owner: `WORK_SHOULD_FIX`
- Problem: TASK-003 references only AC-028/AC-029 and prose tests. Admin operations
  and audit persistence are later-task behavior, so the task needs an exact observable
  seam that proves fail-closed routing without implementing those later capabilities.
- Minimum action: accept §8–§10 and add the IDs/start record canonically.
- Verification: document traceability rejects TASK-003 `IN_PROGRESS` until every ID
  exists and the start record is synchronized.

## 4. Task-order decision

### Option A — TASK-004 before TASK-003 (**recommended**)

1. Keep TASK-003 `PENDING` and production code untouched.
2. Establish and approve TASK-004's own stable AC/TEST registry and start record.
3. Implement only TASK-004 bootstrap/store scope: durable `LibraryOwnerUid`, Library
   lock, accepted SQLite runtime and migration bootstrap.
4. Return to TASK-003. Its composition root receives an already-open authenticated
   Library context containing the durable owner UID; framing/IPC crates never depend
   on SQLite.

Benefits: satisfies ADR-0004 literally, provides safe stale-socket/lifecycle ownership,
and avoids temporary authority or cross-layer persistence. TASK-004 already depends
only on completed TASK-002, so no dependency cycle is introduced.

Required canonical revision: change the execution order/current-next-task statements;
TASK-007 still depends on both TASK-003 and TASK-006, so downstream semantics do not
change.

### Option B — component-only TASK-003 before TASK-004

TASK-003 may implement framing, proto and peer-policy components using an explicitly
injected `LibraryOwnerUid`, plus a test-only daemon. The production daemon listener,
CLI product command, endpoint cleanup and completion claim remain blocked until
TASK-004 supplies durable owner/lock context.

Consequence: TASK-003 cannot honestly become `DONE` at the end of its own session
under the current acceptance wording. The plan must represent a split task such as
TASK-003A/TASK-003B or move the activation acceptance to TASK-004.

### Rejected option — infer owner from daemon eUID/config/request

Rejected because it weakens `API-008`, `SEC-013`, DEC-017, DEC-021 and ADR-0004, or
makes an untrusted configuration/request value an authentication root.

## 5. Proposed wire contract

This section is proposed for later acceptance regardless of task order.

- Namespace: exact proto3 package `mengxia.core.v1`.
- Framing: exactly four unsigned big-endian length bytes followed by one proto payload.
- Length is payload bytes only. Zero is invalid. The configured maximum is inclusive.
- Reader consumes the four-byte header first, rejects zero/overflow/cap+1 before
  allocating the payload, and never attempts resynchronization after malformed input.
- One connection begins with exactly one `ClientHello`; any other first message fails
  and the server closes after at most one bounded safe error frame.
- Protocol version at this gate is major 1, minor 0. Client offers one major and an
  inclusive minor range; server selects the highest common minor. No common version
  returns `UNSUPPORTED_CAPABILITY` and closes without creating principal/command state.
- `ClientHello.request_id` is an exact canonical UUIDv7 string. Field 3 is reserved;
  injected wire tag 3 is never actor evidence.
- `ServerHello` echoes `request_id`, contains a server-generated canonical UUIDv7
  `correlation_id` and the selected version. It never returns UID/GID or Admin status.
- Handshake has no durable side effect and is safe to retry. It has no `command_id`,
  Project context, pagination, persistence, audit write, Provider/Plugin or media data.
- The common semantic `CommandEnvelope` is not populated with provisional product
  operations in this task. Later owning tasks add operation oneof fields without
  renumbering/reusing published fields; its actor field remains reserved.
- Malformed protobuf, invalid ID, invalid version/range, duplicate hello, unexpected
  message and oversized frame produce static safe errors without reflecting raw bytes.

## 6. Proposed bounded lifecycle

- `MENGXIA_MAX_FRAME_BYTES`: existing default 4 MiB; accepted 64 KiB–16 MiB.
- Proposed `MENGXIA_CLIENT_HANDSHAKE_TIMEOUT_MS`: default 5,000 ms; accepted
  tightening-only range 100–5,000 ms.
- Proposed `MENGXIA_MAX_PENDING_HANDSHAKES`: default 32; accepted range 1–256.
- The connection permit is acquired before spawning/servicing a handshake. Saturation
  returns typed `BACKPRESSURE` when a bounded response is possible, otherwise closes.
- Disconnect cancels the connection-local future. Graceful daemon shutdown stops
  accept, cancels and joins all tracked handshakes within the same bounded deadline.
- No retry loop exists inside the server handshake. Client retry is caller-controlled
  and bounded by its command deadline.
- These values are abuse-resistance caps, not performance SLOs. Widening them requires
  a recorded decision and regression tests.

## 7. Proposed dependencies and code generation

Exact proposed pins, all default features disabled:

```toml
prost = { version = "=0.14.4", default-features = false, features = ["derive"] }
prost-types = { version = "=0.14.4", default-features = false }
prost-build = { version = "=0.14.4", default-features = false } # build only
tokio = { version = "=1.53.1", default-features = false, features = ["io-util", "macros", "net", "rt-multi-thread", "signal", "time"] }
```

- No `tonic`, HTTP, TCP, Serde, SQLite, CAS, `tokio-util`, `nix` or direct `libc` API.
- `tokio::net::UnixStream::peer_cred()` is the only peer-credential API used by
  TASK-003; MengXia production crates add no unsafe FFI.
- The canonical `.proto`, a pinned `FileDescriptorSet` and generated Rust are committed.
- Normal/offline builds compile from the committed descriptor and do not discover or
  execute ambient `protoc`.
- Schema regeneration is an explicit repository command using an exact approved
  `protoc` release/checksum. Network/tool unavailability is `UNVERIFIABLE`, not PASS.
- `TEST-SUPPLY-003` must verify exact pins/features/licenses/advisories and confirm no
  unexpected default feature or duplicate reaches the production graph.

The exact `protoc` release/checksum remains a decision item; this proposal does not
select or download it.

## 8. Authority and endpoint contract

- IPC is arm64 macOS Unix-domain socket only. TCP/loopback and cross-platform claims
  are compile-time/runtime disabled.
- The listener receives `LibraryOwnerUid` from an authenticated opened Library context;
  it never reads the value from wire, CLI argument or environment.
- The endpoint lives directly inside a Core-created owner-only runtime directory.
  Directory mode is 0700; socket mode is owner-only; symlinks and ownership mismatch
  fail before bind/accept.
- Peer credentials are read before protobuf allocation/decoding. Missing credentials
  or UID mismatch closes the connection before hello dispatch, principal construction,
  command claim or state mutation.
- `PrincipalContext` is private server state. Ordinary Client is the only constructible
  authority in TASK-003.
- No Admin listener is created while OQ-010 is open. The Client dispatch registry has
  no privileged operation handler. A direct test seam attempting `AuthorityDomain::Admin`
  returns `ADMIN_AUTH_UNAVAILABLE` before invoking a handler or recording intent.
- TASK-003 makes no tenant-isolation claim and has no Project-scoped product command.
- Endpoint lock/stale-socket recovery is owned by the opened Library lifecycle; this
  is another reason Option A is preferred.

## 9. Proposed stable acceptance IDs

Existing obligations retained:

- `AC-028`: actor-tag/request spoofing cannot affect the channel-derived principal;
  the authenticated dispatch context contains only the derived identity.
- `AC-029`: Admin routing is unavailable to ordinary Client and invokes no privileged
  handler or state/intent writer.

New proposed obligations:

- `AC-060`: frame codec rejects zero, malformed, truncated and cap+1 input before
  payload allocation; cap−1 and cap round-trip exactly.
- `AC-061`: proto package, reserved fields and version negotiation are deterministic;
  request/correlation IDs are canonical UUIDv7 and wire values never become authority.
- `AC-062`: real macOS UDS handshake constructs ordinary Client context only after
  peer UID matches durable `LibraryOwnerUid`; missing/mismatched evidence fails first.
- `AC-063`: CLI reaches the handshake only through the daemon UDS; neither binary nor
  dependency graph opens SQLite/CAS or a TCP listener.
- `AC-064`: handshake timeout, concurrency saturation, disconnect and daemon shutdown
  remain bounded, cancel and join all connection work, and create no durable effect.

## 10. Proposed stable Test IDs

- `TEST-PROTO-001`: descriptor/package/field/reservation/golden-codegen compatibility.
- `TEST-FRAME-001`: property/fuzz-style framing matrix including allocation-before-cap
  prevention and cap−1/cap/cap+1.
- `TEST-HANDSHAKE-001`: version/ID/message-order/error/timeout/disconnect behavior.
- `TEST-IPC-MACOS-001`: real same-UID `peer_cred` integration plus deterministic
  missing/mismatched-owner negative policy tests on arm64 macOS.
- `TEST-AUTH-001`: actor-tag spoof, principal injection and Admin fail-closed seam.
- `TEST-CLI-001`: daemon/CLI UDS handshake observable behavior and safe exit codes.
- `TEST-ARCH-003`: dependency direction, no DB/CAS/TCP/Plugin/Admin surface, no unsafe
  MengXia IPC code and no later operation implementation.
- `TEST-SUPPLY-003`: exact dependency/compiler descriptor policy, licenses and current
  advisories with fail-closed unavailable behavior.
- `TEST-DOC-003`: TASK-003 registry/start/completion/current-state traceability and
  negative stale/unknown-ID/task-order fixtures.

The real cross-UID process fixture requires a privileged CI account matrix that the
current local host does not provide. TASK-003 may not claim that exact OS-level denial
test passed unless such a fixture actually runs. The deterministic mismatch test and
real same-UID credential retrieval are minimum local evidence; release support claims
must retain the privileged negative fixture requirement.

## 11. Proposed TASK-003 start record (not active)

This record is a template only. It MUST NOT be copied into the canonical plan or mark
TASK-003 `IN_PROGRESS` until the task-order decision, finite caps, protoc policy,
AC/TEST definitions and baseline verification are accepted.

- Scope: TASK-003 framing/local Client IPC only; no persistence, migrations, product
  operations, Admin listener, Plugin channel, Broker, Provider, TCP or later behavior.
- Feature/Requirements: `FUNC-001`; `API-001`; `API-002`; `API-003`; `API-008`;
  `API-009`; `API-010`; `SEC-005`; `SEC-013`; `SEC-014`; `SEC-017`; `SEC-021`;
  `REL-006`; `CFG-001`; `CFG-003`.
- Decisions: `BASE-007`; `BASE-008`; `BASE-012`; `BASE-013`; `BASE-014`;
  `BASE-016`; `DEC-007`; `DEC-012`; `DEC-016`; `DEC-017`; `DEC-019`;
  `DEC-021`; `DEC-022`; `ADR-0001`; `ADR-0004`; `ADR-0005`.
- Acceptance obligations: `AC-028`; `AC-029`; `AC-060`; `AC-061`; `AC-062`;
  `AC-063`; `AC-064`.
- Verification obligations: `TEST-PROTO-001`; `TEST-FRAME-001`;
  `TEST-HANDSHAKE-001`; `TEST-IPC-MACOS-001`; `TEST-AUTH-001`;
  `TEST-CLI-001`; `TEST-ARCH-003`; `TEST-SUPPLY-003`; `TEST-DOC-003`.

## 12. Required user decision

Task-order decision result:

- **Option A (accepted):** revise the plan so TASK-004 gate/implementation runs
  before TASK-003; keep this TASK-003 contract draft for later confirmation.
- **Option B (not selected):** keep the order but split TASK-003 into component-only and activation
  tasks; it cannot be marked fully DONE before the durable owner/lock integration.

Separately, before TASK-003 implementation, confirm or revise the proposed framing,
finite lifecycle caps and descriptor-based pinned protoc policy. No production code
should begin merely because this draft exists.
