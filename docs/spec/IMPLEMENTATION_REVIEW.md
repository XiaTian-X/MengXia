---
title: "梦夏（MengXia）实现可行性与安全能力审查"
project: "梦夏 / MengXia"
document_role: "Independent Implementation and Security Review"
status: "TASK_002_VERIFIED_WITH_LATER_GATES"
version: "1.1.7"
date: "2026-08-21"
reviewed_spec: "IMPLEMENTATION_SPEC.md v1.1.7"
---

# 梦夏实现可行性与安全能力审查

本记录审查的是“一个新的 Codex 仅依据仓库入口文档能否安全、确定地实现 V1”，不是对文案质量的评价。Current State 已包含 TASK-001 的 Cargo workspace、17 个 package/binary skeleton、CI 与 repository verification infrastructure，以及 TASK-002 已验证的 foundation value/error baseline；schema、migration、IPC、SQLite runtime、CAS 或产品能力仍不存在。Target State 为规范定义的完整系统。Feature matrix 仍评价规范可实现性，不能把 foundation types 或代码存在误报为后续功能已完成。

## 1. Readiness verdict

| Dimension | Verdict | Reason |
|---|---|---|
| Functional readiness | `CONDITIONALLY READY` | TASK-001..TASK-005 foundation path is specified, but blocked later features mean full V1 is not unconditionally ready. |
| Security readiness | `CONDITIONALLY READY` | fail-closed foundation controls are specified; Admin, third-party Native Plugin, Credential, egress and destructive flows remain disabled behind unresolved gates. |
| Codex implementation readiness | `NOT READY FOR CODEX` | this is the required whole-V1 verdict because blocked features/open decisions remain; it neither invalidates the verified TASK-001/TASK-002 slice nor authorizes a later task. |

Current verified completed slice: `TASK-001 DONE`; `TASK-002 DONE`. The accepted `REVIEW-GAP-003` contract and synchronized completion evidence close TASK-002 without widening its scope. No TASK-003 or later capability is authorized. A later capability MUST remain disabled while its own BLOCKER/OQ or task-start gate is open; TASK-002 completion is not whole-V1 readiness or later-feature completion evidence.

## 2. Feature Realizability Matrix

| Feature ID | Feature | Required Components | Data | Interfaces | Failure Handling | Tests | Status |
|---|---|---|---|---|---|---|---|
| `FUNC-001` | Library 初始化、打开、迁移、恢复 | daemon, config, store, migration | LibraryMeta, schema history, ownership | one-shot bootstrap, daemon open, status/health | target/lock/schema/FS/SQLite failure | AC-050..AC-054; TASK-001 TEST registry; later migration/recovery registry before task start | `IMPLEMENTABLE` |
| `FUNC-002` | Managed Asset ingest | CLI, Core API, app, CAS, store | Asset graph, CommandRecord, events | ingest/inspect | source race, disk full, orphan recovery | AC-001..AC-006 | `PARTIALLY_SPECIFIED` |
| `FUNC-003` | Asset 查询与 materialize | API, policy, storage broker | representations, locations, materialization record | inspect/materialize/list | missing/corrupt/denied/quota | contract + path/security | `PARTIALLY_SPECIFIED` |
| `FUNC-004` | Project/Work/Take 创作闭环 | domain, app, store | ProjectSpecRevision, WorkRevision, Take | create/revise/transition/query | conflict/invalid transition | AC-010..AC-011 | `PARTIALLY_SPECIFIED` |
| `FUNC-005` | Recipe 计划与 Run 执行 | resolver, runtime, queues, store | plan/run/step/attempt/job | register/plan/start/status/cancel/retry/resume | partial failure, crash, cancellation | AC-012..AC-014, AC-031 | `PARTIALLY_SPECIFIED` |
| `FUNC-006` | Plugin package 安装、授权、撤销 | admin API, package, policy, host | package, grant, diff, revocation, audit | acquire/inspect/approve/activate/revoke | tamper/revocation/protocol | AC-020, AC-027 | `BLOCKED` |
| `FUNC-007` | Native Plugin containment 与 Broker | platform sandbox, leases, brokers | evidence, leases, audit | private control/broker protocols | backend missing/escape/quota | AC-020..AC-026 + hostile suite | `BLOCKED` |
| `FUNC-008` | Provider submit/inspect/collect/recovery | provider port, network/secret broker, runtime | external operation, observations | provider lifecycle contract | unknown submit/outage/rate limit | AC-013..AC-014, AC-030..AC-031 | `BLOCKED` |
| `FUNC-009` | Provenance、Rights、Usage clearance | domain, policy, store | assertions/context/decision/events | record/query/correct/evaluate | conflicted/unknown evidence | scoped decision tests | `PARTIALLY_SPECIFIED` |
| `FUNC-010` | Audit、verify 与安全诊断 | store, observability, doctor | append-only audit, issues | audit query/export, verify, doctor | corruption/redaction failure | security + integrity tests | `PARTIALLY_SPECIFIED` |
| `FUNC-011` | Retire、Location removal、GC、Purge | admin policy, storage, store | reachability, holds, tombstones | distinct destructive commands | interrupted purge/last-copy risk | destructive/fault tests | `BLOCKED` |
| `FUNC-012` | Provider-neutral portability | ports, adapters, schemas | bindings/extensions | common adapter contract | missing plugin/version skew | AC-030 + contract suite | `PARTIALLY_SPECIFIED` |

`BLOCKED` 表示实现所需事实或决策尚不存在；`PARTIALLY_SPECIFIED` 表示链路仍有接口、状态或验收缺口。当前没有任何 feature 可标记为已实现。

## 3. Findings

### REVIEW-016 — RESOLVED

Severity: `BLOCKER`

Category: Task acceptance / test traceability

Affected feature: `FUNC-001`

Affected requirement: `SEC-020`, `DATA-006`, Specification §0.5 task traceability contract

Location: Specification §18 `TASK-001`; `IMPLEMENTATION_PLAN.md` TASK-001 row

Problem: TASK-001 used mutable natural-language phrases—“workspace builds”, “architecture test” and “smoke build”—but referenced no stable acceptance or test identifiers. The first correction then required every active TEST ID to map to a repository command before TASK-001 started, even though creating those commands is part of TASK-001 itself.

Why implementation may fail: Codex could either mark repository bootstrap complete after only a happy-path build or be forbidden from starting the task that creates the required checks.

Security impact: dependency-direction negative tests, canonical naming, supply-chain fail-closed behavior or document traceability could be omitted, or the security bootstrap could be bypassed to break the circular gate.

Reliability impact: task state could not advance deterministically from `PENDING / NEXT` to `IN_PROGRESS` and `DONE`.

Evidence: Specification v1.1.3 §0.5 required repository command mapping for every active TEST ID while TASK-001's Goal/Implementation owned creation of the repository checks.

Required correction: define immutable `AC-*` and `TEST-*` identifier rules; add TASK-001 acceptance IDs AC-050, AC-051, AC-052, AC-053 and AC-054 plus executable obligations TEST-BOOT-001, TEST-BOOT-002, TEST-ARCH-001, TEST-NAME-001, TEST-SUPPLY-001 and TEST-DOC-001; require stable obligation references before start, but repository command mapping and per-ID PASS evidence only before DONE.

Verification after correction: a simulated task-start record is valid with the stable IDs and no pre-existing command; a simulated DONE record fails until all six TEST IDs map to deterministic commands and report PASS.

Resolution evidence: Specification v1.1.4 §0.5, §18 TASK-001, §19.0 and §20.0; Plan v0.3.3 §4 and TASK-001 row.

Status: `RESOLVED`. TASK-001 now has the required start/completion records and fresh executable PASS evidence for every listed ID; the same staged rule remains mandatory for every later task.

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

Evidence: Specification v1.0.1 listed enterprise RBAC as a non-goal and used Project policy/context, but did not define a Library-wide single-owner trust domain or state that Project is not a tenant/Asset owner.

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

Evidence: IMPLEMENTATION_PLAN v0.1.0 contained phase prose without stable TASK rows, requirement/acceptance mapping, affected files, negative tests or explicit do-not-change constraints.

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

Evidence: Specification v1.0.1 §10.2 exposed only selected vertical-slice examples and omitted the Work/Take, Run control, Plugin Admin, Credential Admin, Rights/Clearance, audit and destructive operation groups later added to the minimum registry.

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

Evidence: Specification v1.0.1 named StepRun/Attempt/Job and lifecycle states but lacked complete persisted shapes/transition tables, and its Blob/Location lifecycle fields did not match the state-machine requirements.

Required correction: persist state/revision for every lifecycle object; define legal transitions, terminal states, concurrency token and recovery action; reject all unspecified transitions.

Verification after correction: exhaustive transition/property tests plus kill tests at every external/destructive boundary.

### REVIEW-006

Severity: `HIGH`

Category: Idempotency / concurrency

Affected feature: every mutation

Affected requirement: `REQ-011`, `REQ-012`, proposed `DATA-009`, `REL-005`

Location: Specification §10.1, AC-005..AC-006

Problem: `command_id` exists but CommandRecord ownership, canonical request digest, in-progress behavior, durable result/error replay, retention and concurrent first-writer race are undefined.

Why implementation may fail: duplicate requests can both execute or permanently alias different actors/operations.

Security impact: replay of sensitive/destructive commands.

Reliability impact: duplicate Provider charges, events and assets.

Evidence: Specification v1.0.1 defined `command_id` in the envelope and duplicate-command acceptance prose but no durable CommandRecord binding, canonical request digest, first-writer claim or stored-outcome replay contract.

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

Evidence: the original task sequence assigned `0001_library_assets.sql` creation to TASK-004 while TASK-006 also needed to add the Asset schema after migration bytes were declared immutable.

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

Evidence: Specification v1.0.1 §16 left frame, queue, buffer and log defaults `TBD`, while the then-current OQ-006 wording did not block the first tasks that consumed those boundaries.

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

Evidence: the original TASK-007 listed copy/adopt/reference ingest modes without defining source deletion, ownership transfer, durability or Managed-custody behavior for adopt/reference.

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

Evidence: the original specification separated an Admin endpoint but left `OQ-004` secret-store selection and the stronger Admin authority/user-presence mechanism undefined; endpoint possession was the only concrete distinction.

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

Evidence: the original Specification §13.5 startup sequence placed full ExternalOperation reconciliation before accepting mutations and did not provide bounded degraded startup behavior when a Provider was offline.

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

Evidence: the original package identity included publisher and a `VERIFIED` state without specifying publisher authentication, and the coding constraints lacked fail-closed license/advisory freshness behavior.

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

Evidence: the original Rights/Clearance and Blob retirement/GC sketches had no complete semantic operation registry, dedicated owning tasks or executable acceptance criteria, while retention/hold policy remained open.

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

Evidence: no concrete Provider or inbound-listener topology was selected, yet the original generic integration text discussed webhook handling without an adapter-specific signature/replay/ordering contract.

Required correction: no webhook listener exists by default. A Provider-specific webhook requires an accepted adapter contract covering signature, timestamp/replay window, secret rotation, deduplication, ordering and size limits; it remains an observation, never recovery authority.

Verification after correction: architecture test proves no TCP listener by default; adapter-specific negative tests are mandatory if enabled.

### REVIEW-017 — RESOLVED

Severity: `HIGH`

Category: Configuration / abuse resistance

Affected feature: `FUNC-001`, `FUNC-002`

Affected requirement: `CFG-001`, `CFG-003`, `SEC-021`, `REL-001`

Location: Specification §16; ADR-0005

Problem: ADR-0005 accepted configurable decode-depth, DB read/busy, ingest-concurrency, single-ingest, aggregate-staging and free-space boundaries, but Specification v1.1.3 §16 omitted their typed keys and did not state which boundaries could only tighten.

Why implementation may fail: one implementer could hard-code the ADR values, another could invent key names/ranges, and another could allow configuration to widen a security maximum despite the decision requirement.

Security impact: configuration drift can silently remove DoS and disk-exhaustion protections.

Reliability impact: overload, busy-timeout and disk-admission behavior becomes deployment-specific and cannot be reproduced by tests.

Evidence: ADR-0005 lists twelve foundation boundaries while v1.1.3 §16 exposed only frame, writer queue, stream buffer and two worker limits.

Required correction: expose every accepted configurable boundary through the typed configuration model, define accepted/tightening-only behavior and reject invalid or widening values before enabling the dependent operation.

Verification after correction: the configuration inventory covers every ADR-0005 boundary; parser tests cover missing, zero, overflow, range endpoints, widening attempts and impossible reserve/staging combinations.

Status: `RESOLVED` in Specification v1.1.4 §16 and clarified ADR-0005.

### REVIEW-018 — RESOLVED

Severity: `HIGH`

Category: Bootstrap state / filesystem authorization

Affected feature: `FUNC-001`

Affected requirement: `DATA-005`, `DATA-006`, `SEC-013`

Location: Specification §18 TASK-004; ADR-0004 Verification

Problem: the accepted bootstrap contract allows an absent or correctly owned empty target, while TASK-004 and ADR verification said an “existing” target must be rejected. An empty existing directory satisfies both statements and therefore had contradictory required outcomes.

Why implementation may fail: separate implementations could accept or reject the same safe first-create target, and tests could demand the opposite of the authority contract.

Security impact: an over-broad fix might accept existing canonical state or unsafe ownership merely because the target is empty-looking; an under-broad fix needlessly prevents safe owner-prepared directories.

Reliability impact: first startup behavior is non-deterministic across installers and manual setups.

Evidence: ADR-0004 Decision says “absent or empty”; its Verification and Specification v1.1.3 TASK-004 said “reject existing/non-empty”.

Required correction: accept only absent or correctly owned/mode empty targets; reject canonical metadata, non-empty content, ownership/mode mismatch, symlink substitution and unsupported filesystems.

Verification after correction: a target-state matrix tests absent, safe-empty, canonical-existing, arbitrary-non-empty, wrong-owner/mode, symlink and non-APFS cases with exactly one expected outcome per row.

Status: `RESOLVED` in Specification v1.1.4 TASK-004 and clarified ADR-0004.

### REVIEW-019 — RESOLVED

Severity: `BLOCKER`

Category: Readiness verdict / implementation authorization scope

Affected feature: all V1 features

Affected requirement: user-mandated final review contract; all open `OQ-*` gates

Location: Review §1 and §6; Plan status

Problem: the review used custom verdicts `READY FOR FOUNDATION`, `READY FOR FAIL-CLOSED FOUNDATION` and `READY FOR TASK-001` in the three required final-verdict rows. Those values did not match the mandated enums and conflated a scoped task authorization with whole-V1 readiness.

Why implementation may fail: a new Codex could read the scoped “ready” result as authority to implement blocked Admin, Plugin, Credential, Provider or destructive capabilities, or could reject safe TASK-001 work because the full product is not ready.

Security impact: fail-closed later gates could be bypassed through an over-broad interpretation of readiness.

Reliability impact: task scheduling and completion claims would depend on which readiness scope an implementer assumed.

Evidence: the original review request permits only `READY / CONDITIONALLY READY / NOT READY` for functional/security and `READY FOR CODEX / NOT READY FOR CODEX` for Codex readiness; Review v1.1.3 used none of those exact row values while the feature matrix still contained `BLOCKED` entries.

Required correction: use the mandated whole-V1 enum values and report `READY FOR TASK-001` separately as the current authorized slice; keep every later task bound to its own OQ/Review/AC/TEST gate.

Verification after correction: the verdict table contains only allowed values; a simulated Codex can start TASK-001 but cannot infer authority for any blocked later capability.

Status: `RESOLVED` in Review v1.1.4 and Plan v0.3.4.

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
| `REVIEW-016` | stable obligation/command lifecycle corrected | declare all TASK-001 AC/TEST IDs before start; commands must exist and pass before DONE |
| `REVIEW-017` | configuration inventory/range semantics corrected | all later Plugin/Provider caps remain gated by their OQ-006 sub-decisions |
| `REVIEW-018` | bootstrap target matrix reconciled | TASK-004 must execute the complete real-filesystem matrix before DONE |
| `REVIEW-019` | whole-V1 verdict separated from task authorization | full V1 remains NOT READY FOR CODEX; TASK-001 and TASK-002 are complete; TASK-003 remains unauthorized pending its own gate |

Second-pass conclusion as of 2026-08-21: ADR-0003..ADR-0005 close the applicable foundation decisions, TASK-001 retains fresh PASS evidence, and REVIEW-GAP-003 plus TASK-002's per-ID evidence close its value/error baseline. The honest whole-V1 verdict remains `FUNCTIONAL: CONDITIONALLY READY`, `SECURITY: CONDITIONALLY READY`, `CODEX: NOT READY FOR CODEX`; the verified completed slice is `TASK-001` and `TASK-002`, while TASK-003 remains unauthorized pending its own stable registry/start gate.

## 7. Codex implementation simulation

### `TASK-001`

Rust/MSRV 1.98.0 and bundled SQLite 3.53.4 are accepted in ADR-0003. Stable acceptance/test IDs and their staged lifecycle are defined by Specification v1.1.5. Result: `DONE`; AC-050, AC-051, AC-052, AC-053, AC-054 and all six TEST obligations have fresh repository PASS evidence.

### `TASK-002`

The accepted value/error public contracts, exact minimal dependency features, AC-055 through AC-059 and seven TEST obligations are canonical, implemented and reproducibly verified. Result: `TASK-002 DONE`; per-ID acceptance/security evidence passes, the complete TASK-001 baseline remains green, and no TASK-003 behavior was added.

### `TASK-003`

The required local transport, server-derived principal, Admin separation, validation and failure behavior are defined. ADR-0004 accepts macOS peer UID and disables Admin; ADR-0005 accepts the frame cap. Result: `PENDING` on its stable AC/TEST registry and start record; TASK-002 is complete and there is no remaining foundation decision blocker.

### `TASK-004`

The migration engine, SQLite hardening and file ownership are identifiable. ADR-0003 accepts the fixed bundled version; ADR-0004 accepts the initial platform/APFS validation scope; ADR-0005 accepts DB queue bounds. Result: `PENDING` on its stable AC/TEST registry and start record; TASK-002 is complete and there is no remaining foundation decision blocker.

### `TASK-005`

The stable-handle CAS primitive, durability ordering, path boundary and accepted stream/I/O/hash/staging caps are identifiable. Result: `PENDING` on its stable AC/TEST registry and start record; TASK-002 is complete and there is no remaining foundation decision blocker.

The simulation and repository evidence confirm `TASK-001 DONE` and `TASK-002 DONE` while the whole-V1 result remains `NOT READY FOR CODEX`. This does not authorize TASK-003, Admin, third-party Native Plugin, Credential, Provider egress, Rights clearance, GC or Purge.
