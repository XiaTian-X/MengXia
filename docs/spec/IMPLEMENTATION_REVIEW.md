---
title: "梦夏（MengXia）实现可行性与安全能力审查"
project: "梦夏 / MengXia"
document_role: "Independent Implementation and Security Review"
status: "READY_FOR_TASK_001_WITH_LATER_GATES"
version: "1.1.1"
date: "2026-08-21"
reviewed_spec: "IMPLEMENTATION_SPEC.md v1.1.2"
---

# 梦夏实现可行性与安全能力审查

本记录审查的是“一个新的 Codex 仅依据仓库入口文档能否安全、确定地实现 V1”，不是对文案质量的评价。Current State 为仅含 Git 元数据和文档的仓库；Target State 为规范定义的系统。仓库中目前没有源代码、schema、migration、测试或基础设施实现，因此所有实现能力均只能评价为规范可实现性，不能评价为已经实现。

## 1. Readiness verdict

| Dimension | Verdict | Reason |
|---|---|---|
| Functional readiness | `READY FOR FOUNDATION` | TASK-001..005 的工具链、普通 Client authority、SQLite 与有限 safety caps 已接受；后期功能仍受各自 gate 约束。 |
| Security readiness | `READY FOR FAIL-CLOSED FOUNDATION` | arm64 macOS ordinary Client 边界与 foundation caps 已接受；Admin、第三方 Native Plugin、Credential、egress 和 destructive flows 保持禁用。 |
| Codex implementation readiness | `READY FOR TASK-001` | ADR-0003..0005 close the decisions that previously blocked repository bootstrap and foundation work. |

Codex may start TASK-001 and then follow dependency order through TASK-005. A later capability MUST remain disabled while its own BLOCKER/OQ is open; foundation readiness is not whole-V1 readiness.

## 2. Feature Realizability Matrix

| Feature ID | Feature | Required Components | Data | Interfaces | Failure Handling | Tests | Status |
|---|---|---|---|---|---|---|---|
| `FUNC-001` | Library 初始化、打开、迁移、恢复 | daemon, config, store, migration | LibraryMeta, schema history, ownership | one-shot bootstrap, daemon open, status/health | target/lock/schema/FS/SQLite failure | bootstrap, migration, crash, corruption | `IMPLEMENTABLE` |
| `FUNC-002` | Managed Asset ingest | CLI, Core API, app, CAS, store | Asset graph, CommandRecord, events | ingest/inspect | source race, disk full, orphan recovery | AC-001..006 | `PARTIALLY_SPECIFIED` |
| `FUNC-003` | Asset 查询与 materialize | API, policy, storage broker | representations, locations, materialization record | inspect/materialize/list | missing/corrupt/denied/quota | contract + path/security | `PARTIALLY_SPECIFIED` |
| `FUNC-004` | Project/Work/Take 创作闭环 | domain, app, store | ProjectSpecRevision, WorkRevision, Take | create/revise/transition/query | conflict/invalid transition | AC-010..011 | `PARTIALLY_SPECIFIED` |
| `FUNC-005` | Recipe 计划与 Run 执行 | resolver, runtime, queues, store | plan/run/step/attempt/job | register/plan/start/status/cancel/retry/resume | partial failure, crash, cancellation | AC-012..014, AC-031 | `PARTIALLY_SPECIFIED` |
| `FUNC-006` | Plugin package 安装、授权、撤销 | admin API, package, policy, host | package, grant, diff, revocation, audit | acquire/inspect/approve/activate/revoke | tamper/revocation/protocol | AC-020, AC-027 | `BLOCKED` |
| `FUNC-007` | Native Plugin containment 与 Broker | platform sandbox, leases, brokers | evidence, leases, audit | private control/broker protocols | backend missing/escape/quota | AC-020..026 + hostile suite | `BLOCKED` |
| `FUNC-008` | Provider submit/inspect/collect/recovery | provider port, network/secret broker, runtime | external operation, observations | provider lifecycle contract | unknown submit/outage/rate limit | AC-013..014, AC-030..031 | `BLOCKED` |
| `FUNC-009` | Provenance、Rights、Usage clearance | domain, policy, store | assertions/context/decision/events | record/query/correct/evaluate | conflicted/unknown evidence | scoped decision tests | `PARTIALLY_SPECIFIED` |
| `FUNC-010` | Audit、verify 与安全诊断 | store, observability, doctor | append-only audit, issues | audit query/export, verify, doctor | corruption/redaction failure | security + integrity tests | `PARTIALLY_SPECIFIED` |
| `FUNC-011` | Retire、Location removal、GC、Purge | admin policy, storage, store | reachability, holds, tombstones | distinct destructive commands | interrupted purge/last-copy risk | destructive/fault tests | `BLOCKED` |
| `FUNC-012` | Provider-neutral portability | ports, adapters, schemas | bindings/extensions | common adapter contract | missing plugin/version skew | AC-030 + contract suite | `PARTIALLY_SPECIFIED` |

`BLOCKED` 表示实现所需事实或决策尚不存在；`PARTIALLY_SPECIFIED` 表示链路仍有接口、状态或验收缺口。当前没有任何 feature 可标记为已实现。

## 3. Findings

### REVIEW-001

Severity: `BLOCKER`

Category: Authentication / authorization

Affected feature: `FUNC-001`..`FUNC-011`

Affected requirement: `SEC-005`, `REQ-010`, proposed `API-008`, `SEC-013`

Location: Specification §10.1, §12.3; `TASK-003`

Problem: `actor_principal` is supplied by the caller while the server-side derivation and binding of Client/Admin principal are not defined. A protected socket path is not itself a complete actor or administrator authentication contract.

Why implementation may fail: one implementation may trust the field, another may replace it with peer UID/SID, and a third may use an undisclosed bearer token. These produce incompatible authorization and audit semantics.

Security impact: identity spoofing, administrative privilege escalation and false audit attribution.

Reliability impact: idempotency ownership and recovery authorization cannot be evaluated consistently.

Evidence: the envelope contains a caller-controlled principal; §12.3 says self-report is untrusted but does not define the authoritative Client/Admin mechanism.

Required correction: reserve/remove the caller actor field; derive `PrincipalContext` from an authenticated channel; explicitly document the single-user trust boundary; block Admin operations until a platform-specific elevation/user-presence mechanism is accepted.

Verification after correction: negative IPC tests spoof the actor, connect from an unauthorized peer, and attempt ordinary-Client access to Admin operations; no command row or domain state changes.

### REVIEW-002

Severity: `HIGH`

Category: Isolation scope

Affected feature: all Project-scoped features

Affected requirement: `REQ-007`, proposed `SEC-014`

Location: Specification §3, §8.2, §12.2

Problem: the document uses Project policy but never states whether Project is a tenant/security boundary. Enterprise RBAC is a non-goal, yet the authorization model can be misread as multi-tenant isolation.

Why implementation may fail: Codex may add incomplete tenant filters or assume Project ownership grants global Asset ownership.

Security impact: false tenant-isolation claims or IDOR between Project contexts.

Reliability impact: inconsistent global Asset references.

Required correction: V1 is a single-Library, single-local-owner trust domain; Project is a policy/work context, never a tenant. Missing/mismatched Project context is denied where a command requires it. Multi-tenant serving is prohibited without a new architecture/version.

Verification after correction: cross-Project tests prove policy checks while global Asset identity remains Library-owned; documentation and API contain no multi-tenant claim.

### REVIEW-003

Severity: `BLOCKER`

Category: Implementation planning / traceability

Affected feature: all

Affected requirement: all P0 requirements

Location: `IMPLEMENTATION_PLAN.md`; Specification §18

Problem: the living plan contains phases but no task IDs, requirement mapping, affected files, security implications, task-specific negative tests or do-not-change constraints. It also marks the canonical spec `DONE` despite unresolved blockers.

Why implementation may fail: task completion does not demonstrate feature completion, and Codex can start work whose prerequisite decision is open.

Security impact: security foundations can be skipped as “later phase” work.

Reliability impact: incomplete task acceptance can be marked done.

Required correction: make Phase 0 a hard documentation/decision gate; add a task traceability index and prohibit task start while any blocking decision is open.

Verification after correction: every task maps to requirements and acceptance/security tests; an automated doc check rejects unknown IDs and unmet dependencies.

### REVIEW-004

Severity: `HIGH`

Category: API completeness

Affected feature: `FUNC-003`..`FUNC-011`

Affected requirement: `REQ-010`, `API-003`, proposed `API-010`

Location: Specification §10.2

Problem: the operation registry does not expose many actions required by the domain and CLI-only V1: Work/Take creation/query, Run status/cancel/retry/reconcile, Plugin lifecycle/grants, Credential administration, Rights/Clearance, audit export, Location removal and GC/Purge.

Why implementation may fail: the domain describes capabilities that no entry point can trigger or observe.

Security impact: implementers may add generic CRUD or internal bypass methods without policy contracts.

Reliability impact: recovery and destructive actions have no stable idempotency/error contract.

Required correction: add a minimum complete semantic operation registry. Each concrete proto must define validation, authorization, side effects, idempotency, deadline and error mapping before implementation.

Verification after correction: CLI/API parity test and operation-contract lint.

### REVIEW-005

Severity: `BLOCKER`

Category: Data model / state machines

Affected feature: `FUNC-005`, `FUNC-006`, `FUNC-011`

Affected requirement: `REQ-009`, `REL-003`, proposed `DATA-010`

Location: Specification §8.3, §9.0, §9.3..9.5

Problem: Blob/Location lifecycle fields do not match their state tables; StepRun/Attempt/Job are named but not shaped; Job and Plugin package diagrams omit authoritative transition/precondition tables; destructive recovery states are not persisted.

Why implementation may fail: two implementations can create different legal transitions and different recovery queries.

Security impact: revoked code can resume, or destructive work can repeat after a crash.

Reliability impact: duplicate effects, state regression and unrecoverable partial collection.

Required correction: persist state/revision for every lifecycle object; define legal transitions, terminal states, concurrency token and recovery action; reject all unspecified transitions.

Verification after correction: exhaustive transition/property tests plus kill tests at every external/destructive boundary.

### REVIEW-006

Severity: `HIGH`

Category: Idempotency / concurrency

Affected feature: every mutation

Affected requirement: `REQ-011`, `REQ-012`, proposed `DATA-009`, `REL-005`

Location: Specification §10.1, AC-005..006

Problem: `command_id` exists but CommandRecord ownership, canonical request digest, in-progress behavior, durable result/error replay, retention and concurrent first-writer race are undefined.

Why implementation may fail: duplicate requests can both execute or permanently alias different actors/operations.

Security impact: replay of sensitive/destructive commands.

Reliability impact: duplicate Provider charges, events and assets.

Required correction: define Library-wide unique CommandRecord keyed by command ID, principal and operation; insert/claim in the mutation transaction; same digest replays stored outcome, different digest/principal conflicts; in-progress returns typed state; no automatic expiry for effectful history before retention policy.

Verification after correction: concurrent duplicate, crash-between-claim-and-result, actor mismatch and payload mismatch tests.

### REVIEW-007

Severity: `HIGH`

Category: Migration safety

Affected feature: `FUNC-001`, `FUNC-002`

Affected requirement: `DATA-007`

Location: Specification §8.7, `TASK-004`, `TASK-006`

Problem: `TASK-004` creates `0001_library_assets.sql`, then `TASK-006` also modifies the same migration even though merged migration bytes are immutable.

Why implementation may fail: normal task sequencing makes a required later edit look like checksum tampering.

Security impact: migration checksum bypass pressure.

Reliability impact: non-reproducible upgrades.

Required correction: TASK-004 builds only the migration engine/bootstrap metadata; TASK-006 owns immutable `0001_library_assets.sql` once its complete schema is reviewed.

Verification after correction: clean install and upgrade fixtures; changing an applied migration fails startup.

### REVIEW-008

Severity: `BLOCKER`

Category: Abuse resistance / safe defaults

Affected feature: IPC, queues, ingest, Plugin runtime

Affected requirement: `REL-001`, `CFG-001`, proposed `CFG-003`

Location: Specification §16, `OQ-006`

Problem: hard frame, queue, buffer and log caps are required for correctness and DoS resistance but all defaults are `TBD`, while OQ-006 incorrectly says they do not block foundation work.

Why implementation may fail: Codex must invent limits or accidentally ship unbounded behavior.

Security impact: memory, disk, process and cost exhaustion.

Reliability impact: overload collapse and untestable backpressure.

Required correction: separate safety caps from performance SLOs. Finite implementation caps must be accepted before the task that consumes them; performance SLOs can remain TBD until benchmark work.

Verification after correction: boundary/overload tests at cap-1/cap/cap+1 and bounded disk/memory assertions.

### REVIEW-009

Severity: `HIGH`

Category: Storage semantics

Affected feature: `FUNC-002`

Affected requirement: `DATA-002`, `DATA-003`

Location: `TASK-007`

Problem: “copy/adopt/reference modes” are listed without ownership, destructive behavior or durability semantics. Reference mode can violate the Managed Asset custody guarantee; adopt can delete/move user data.

Why implementation may fail: different implementations may register non-durable assets or destructively move source files.

Security impact: path authority expansion.

Reliability impact: data loss and broken Asset references.

Required correction: V1 `IngestAsset` supports copy mode only until separate semantic contracts are accepted. Reference is an unmanaged external Location and cannot satisfy Managed custody; adopt requires an explicit destructive command and is outside the initial vertical slice.

Verification after correction: API rejects unknown modes; source is never removed; canonical registration always points to verified durable custody.

### REVIEW-010

Severity: `BLOCKER`

Category: Secrets / privileged operations

Affected feature: `FUNC-006`, `FUNC-008`

Affected requirement: `SEC-005`, `SEC-007`, `CFG-002`

Location: Specification §10.4, §12.3, §12.6; `OQ-004`

Problem: secret-store selection is open and the Admin authority mechanism is unspecified. A separate endpoint alone does not create higher privilege when the same caller can open it.

Why implementation may fail: Credential configuration or Plugin grant approval can become ordinary same-user RPCs.

Security impact: credential theft and privilege escalation.

Reliability impact: rotation/revocation behavior is inconsistent.

Required correction: block Credential and Admin-sensitive implementation until the target platform, approved secret store and Admin authentication/user-presence mechanism are accepted. Secret material is never stored in SQLite; only opaque references and safe metadata are canonical.

Verification after correction: secret-store integration tests, denied ordinary-client calls, rotation/revocation, log/event/database scans for canary secrets.

### REVIEW-011

Severity: `HIGH`

Category: Startup recovery / availability

Affected feature: `FUNC-001`, `FUNC-005`, `FUNC-008`

Affected requirement: `REL-004`, proposed `REL-008`

Location: Specification §13.5

Problem: startup orders full ExternalOperation reconciliation before accepting mutations. A Provider outage can therefore block all unrelated local work indefinitely.

Why implementation may fail: recovery depends on an unavailable external system despite local-first goals.

Security impact: remote denial of service.

Reliability impact: global unavailability.

Required correction: synchronously establish durable scheduler ownership and enqueue bounded reconciliation; after local invariants pass, expose read operations and unrelated mutations in degraded mode. Only affected Runs remain blocked.

Verification after correction: restart with Provider offline permits local ingest/query while affected operations remain durable and visible.

### REVIEW-012

Severity: `HIGH`

Category: Supply chain / executable identity

Affected feature: `FUNC-006`, `FUNC-007`

Affected requirement: `SEC-009`, proposed `SEC-016`, `SEC-020`

Location: Specification §1.1, §9.5, §17

Problem: `publisher` is part of execution identity, but “VERIFIED” does not state whether publisher authenticity is cryptographically proven, locally asserted or merely manifest text. Dependency admission also lacks an enforceable advisory/license policy and stale-advisory behavior.

Why implementation may fail: a package can self-claim a trusted publisher name, or a build can silently accept a known-vulnerable dependency.

Security impact: supply-chain impersonation and vulnerable code execution.

Reliability impact: irreproducible package selection.

Required correction: until a signature/trust-root ADR exists, publisher text is untrusted metadata; authorization binds exact digest plus local Admin grant. “Verified” means bytes/schema/digest/dependency identity only. Add locked dependency review and vulnerability/advisory policy before release.

Verification after correction: publisher spoof, digest substitution, stale/revoked dependency and advisory policy tests.

### REVIEW-013

Severity: `HIGH`

Category: Product closure / destructive operations

Affected feature: `FUNC-009`, `FUNC-011`

Affected requirement: `G-009`, `REQ-005`, proposed `SEC-018`

Location: Specification §8.5, §9.0, §10.2, §18

Problem: Rights/Clearance and destructive lifecycle are domain sketches without complete commands, task ownership or acceptance criteria. Retention is open, so safe GC cannot be implemented.

Why implementation may fail: goals cannot be reached from CLI/API, and cleanup may infer destructive behavior.

Security impact: unauthorized deletion or unreviewed egress.

Reliability impact: leaked storage or permanent data loss.

Required correction: add dedicated post-foundation tasks and semantic operation contracts. Keep Purge disabled until retention/hold policy is accepted; retire/remove/GC/purge remain distinct.

Verification after correction: CLI/API E2E for Rights; last-copy/hold/active-lease denials; crash-safe purge journal tests.

### REVIEW-014

Severity: `BLOCKER`

Category: Dependency security / SQLite

Affected feature: `FUNC-001`

Affected requirement: `DATA-006`, `OQ-003`

Location: toolchain/SQLite decision gate

Problem: the exact SQLite version is open. SQLite's official WAL documentation now records a WAL-reset corruption bug affecting versions through 3.51.2, fixed in 3.51.3 and selected backports. A generic “current supported version” instruction is insufficient for a WAL + read-pool design.

Why implementation may fail: Codex may pin a known-affected bundled SQLite version.

Security impact: integrity/availability degradation through vulnerable dependency selection.

Reliability impact: rare database corruption under concurrent checkpoint/write conditions.

Evidence: SQLite official WAL documentation, §11 (checked 2026-08-20): https://sqlite.org/wal.html

Required correction: `OQ-003` must select a version containing the official WAL-reset fix and record compile options/source/checksum; CI must assert runtime version and compile options.

Verification after correction: bootstrap test rejects an unapproved runtime version; concurrency/checkpoint regression test is retained where practical.

### REVIEW-015

Severity: `MEDIUM`

Category: Webhook scope

Affected feature: `FUNC-008`

Affected requirement: `API-006`

Location: Specification §11.5, §21

Problem: webhook security behavior is described generically although no webhook Provider is selected and no endpoint is in V1's local transport topology.

Why implementation may fail: Codex may create a generic public listener prematurely.

Security impact: unnecessary remote attack surface.

Reliability impact: conflicting recovery authority.

Required correction: no webhook listener exists by default. A Provider-specific webhook requires an accepted adapter contract covering signature, timestamp/replay window, secret rotation, deduplication, ordering and size limits; it remains an observation, never recovery authority.

Verification after correction: architecture test proves no TCP listener by default; adapter-specific negative tests are mandatory if enabled.

## 4. Threat model

### Assets and boundaries

- Assets: canonical identities/history, media bytes, Credentials, Plugin grants/leases, Provider operations, Rights evidence and audit history.
- Actors: local Library owner, ordinary Client/Agent, Admin-authorized session, Core daemon, sandboxed Plugin, trusted first-party executable, Provider and hostile local/external input.
- Trust boundaries: Client IPC, Admin authorization, Core/SQLite, Core/CAS, PluginContainer, Broker channels, Provider network, secret store and imported filesystem.
- V1 limitation: this is not a multi-tenant service and does not claim containment against arbitrary already-compromised software running with the full Library owner's OS authority. It MUST still contain Plugins and reject unauthenticated/misattributed IPC.

| Threat | Asset | Attack Path | Mitigation | Verification |
|---|---|---|---|---|
| Caller identity spoof | state/audit | caller-supplied actor field | channel-derived PrincipalContext; deny mismatch | IPC negative tests |
| Plugin escape | DB/CAS/Credential | native process reads host resources | exact-backend sandbox-or-deny | real hostile suite |
| Broker confused deputy | assets/Credential | valid lease uploads wrong bytes/destination | caller/run/digest/destination-bound lease | cross-run/redirect tests |
| Duplicate paid submit | account/canonical state | timeout then blind retry | durable intent + idempotency + UNKNOWN reconcile | kill/timeout tests |
| Provider success/local failure | output custody | remote complete, DB/register fails | staged durable state and resumable collect/register | AC-031 |
| Path/symlink race | host files | untrusted source/output path | stable handles, root confinement, no shell | path attack tests |
| Resource exhaustion | availability/cost | oversized frames/jobs/logs/output | accepted hard caps, quotas, backpressure | overload/abuse tests |
| Supply-chain spoof | code execution | manifest claims publisher/dependency | digest-bound local grant; verified dependency identity | tamper/spoof tests |
| Secret leakage | Credential | logs/events/CLI/env/raw Plugin access | broker application, secret store, redaction scans | canary scans |
| Unsafe purge | media/history | race/duplicate destructive command | explicit Admin command, holds, expected revision, purge journal | crash/concurrency tests |
| SQLite corruption | metadata | affected version/network FS/checkpoint race | fixed pinned bundle, local FS, checks/assertions | version/bootstrap/recovery tests |

## 5. Attack-path combinations

1. Caller-controlled actor + ordinary access to Admin endpoint + non-actor-bound command ID could produce unaudited privilege escalation. Corrections require channel identity, Admin gate and actor-bound idempotency together.
2. Mutable path upload + redirect-following authorization + raw Credential would turn the Broker into an exfiltration proxy. Corrections require immutable handles, per-hop authorization and Level A/B Credentials together.
3. Undefined retention + reachability race + retryable purge could delete the last durable copy. Corrections require accepted retention/hold policy, serialized mark/sweep snapshot and a non-repeatable purge journal.
4. Unbounded protocol/log queues + hostile Plugin process fan-out can exhaust both Core memory and disk even if filesystem reads are sandboxed. Resource enforcement and application backpressure are both required.

## 6. Second-pass verification status

The 2026-08-20 correction pass updated the canonical documents to make the above gaps explicit and fail closed. At that time it did not fabricate the missing platform, toolchain, secret-store, Admin-auth or safety-limit decisions. Consequently, for that pass:

- no original BLOCKER is silently declared resolved;
- implementation remained stopped at Phase 0 until the 2026-08-21 accepted decisions;
- security controls that depend on unavailable facts are represented as gates;
- non-applicable multi-tenancy and webhook behavior are explicitly scoped;
- task/requirement/acceptance traceability is added to the living plan;
- the migration ownership and copy-only ingest contradictions are corrected.

### Finding disposition after correction

| Review ID | Disposition | Remaining gate |
|---|---|---|
| `REVIEW-001` | foundation closed by ADR-0004 and v1.1.2 bootstrap clarification | one-shot Library bootstrap and ordinary arm64 macOS Client peer-auth accepted; Admin disabled until later OQ-010 decision |
| `REVIEW-002` | corrected | none for single-owner scope; any future multi-tenant version needs new architecture |
| `REVIEW-003` | corrected | Phase 0 must close task-specific gates before code |
| `REVIEW-004` | operation inventory corrected; concrete proto contracts remain task gates | `API-010` review before each operation group |
| `REVIEW-005` | lifecycle persistence, terminal/hold/reconcile rules corrected | retention policy still blocks destructive lifecycle enablement |
| `REVIEW-006` | CommandRecord binding/concurrency/recovery contract corrected | retention decision before any expiry policy |
| `REVIEW-007` | corrected | none; applied-migration immutability remains mandatory |
| `REVIEW-008` | foundation values accepted by ADR-0005 | later Plugin/Provider/release OQ-006 portions remain gated |
| `REVIEW-009` | corrected to copy-only initial ingest | later reference/adopt needs separate contract/ADR |
| `REVIEW-010` | foundation fail-closed decision accepted | `OQ-004`/`OQ-010` remain later blockers; Admin/Credential features disabled |
| `REVIEW-011` | corrected to bounded degraded startup | numeric reconciliation budgets remain an `OQ-006` gate |
| `REVIEW-012` | publisher/digest and dependency policy corrected | signature/trust roots optional only if no authenticity claim is made |
| `REVIEW-013` | API/tasks/AC added; unsafe Purge disabled | `OQ-008`/`OQ-009` remain blockers |
| `REVIEW-014` | closed for foundation by ADR-0003 | runtime/source/options assertions remain mandatory in TASK-004 |
| `REVIEW-015` | corrected; no webhook listener by default | adapter-specific contract if later enabled |

Second-pass conclusion as of 2026-08-21: ADR-0003..0005 close the remaining gates applicable to TASK-001..005. Later unresolved items remain explicit fail-closed gates, so the honest readiness is `READY FOR TASK-001`, not whole-V1 READY.

## 7. Codex implementation simulation

### `TASK-001`

Rust/MSRV 1.98.0 and bundled SQLite 3.53.4 are accepted in ADR-0003. Result: `READY`; TASK-001 is the next implementation task after tool verification.

### `TASK-002`

The value types, trust classification, error families and foundation serialization limits are identifiable. Result: `PENDING` only on TASK-001 dependency.

### `TASK-003`

The required local transport, server-derived principal, Admin separation, validation and failure behavior are defined. ADR-0004 accepts macOS peer UID and disables Admin; ADR-0005 accepts the frame cap. Result: `PENDING` on TASK-002, with no remaining foundation decision blocker.

### `TASK-004`

The migration engine, SQLite hardening and file ownership are identifiable. ADR-0003 accepts the fixed bundled version; ADR-0004 accepts the initial platform/APFS validation scope; ADR-0005 accepts DB queue bounds. Result: `PENDING` on TASK-002, with no remaining foundation decision blocker.

The simulation now confirms `READY FOR TASK-001`. This does not enable Admin, third-party Native Plugin, Credential, Provider egress, Rights clearance, GC or Purge.
