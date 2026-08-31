---
title: "梦夏（MengXia）实施计划"
project: "梦夏 / MengXia"
document_role: "Living Implementation Plan"
status: "TASK_007_IN_PROGRESS"
version: "0.3.36"
date: "2026-08-31"
language: "zh-CN"
source_of_truth: "IMPLEMENTATION_SPEC.md v1.1.25"
review: "IMPLEMENTATION_REVIEW.md v1.1.36"
---

# 梦夏（MengXia）实施计划

本文档记录实施顺序、task gate 和验收证据。架构/语义以 `IMPLEMENTATION_SPEC.md` 为准，开放决策与例外以 `DECISIONS.md`/ADR 为准，当前可实现性与阻塞项以 `IMPLEMENTATION_REVIEW.md` 为准。

## 1. Status semantics

- `PENDING`: 依赖已知但尚未开始。
- `IN_PROGRESS`: 已满足 gate，正在实施；同一阶段最多一个状态 owner。
- `BLOCKED`: 存在明确未解决依赖/决策/Review finding；不得写依赖该项的实现。
- `DONE`: 实现、正向/负向/故障测试及证据均已完成。

Task 不得仅因文件存在或 happy-path 通过而标记 `DONE`。每个 task 完成记录必须列出 commit/worktree state、运行命令、结果、未执行测试及原因；security/recovery test 不得静默跳过。

## 2. Current State versus Target State

| Item | Current State | Target State | Classification |
|---|---|---|---|
| Git | initialized, branch `main` | retained | `FACT` |
| Source/workspace | TASK-001 Cargo workspace and TASK-002 foundation value/error baseline are implemented and verified | domain/runtime behavior implemented by owning tasks | `FACT / PARTIAL TARGET` |
| Schema/migrations | TASK-004 bootstrap migration `0000_store_bootstrap` and TASK-006 immutable `0001_library_assets` with exact current-prefix validation are implemented and verified | reviewed forward-only migrations | `FACT / VERIFIED PREFIX` |
| Tests/CI | TASK-001 repository verification tests/scripts and arm64 macOS CI present | layered functional/security/recovery suites added by owning tasks | `FACT / PARTIAL TARGET` |
| Review | TASK-001, TASK-002, TASK-004, TASK-003, TASK-005 and TASK-006 are implemented with retained local/formal evidence; TASK-007 is active under its accepted independent start record | retain reproducible evidence; activate any later task only through its explicit independent start record | `FACT / VERIFIED / DECISION` |
| Phase 0 decisions | OQ-003, early OQ-006 and foundation Client/Admin boundary accepted | retained until superseded | `DECISION / ACCEPTED` |

Current plan state: `TASK_007_IN_PROGRESS`. TASK-001, TASK-002, TASK-004, TASK-003,
TASK-005 and TASK-006 are verified complete. Specification v1.1.25, ADR-0008 and
accepted TASK-006 proposal v0.2.2 retain the Asset domain, durable command/event
persistence and immutable migration 0001 contract plus reviewed formal run
`33257331689`. ADR-0009 and accepted TASK-007 proposal v0.1.4 now authorize only the
copy-ingest slice. Current implementation authority is `TASK_007_ONLY`; Admin,
root-rebind, TCP/HTTP, Provider/Plugin and TASK-008+ behavior remain unauthorized.

TASK003_CANONICAL_GATE: ACCEPTED
TASK003_SPECIFICATION_VERSION: 1.1.17
TASK003_LIFECYCLE: DONE
TASK003_PROPOSAL: docs/proposals/TASK-003-GATE-PROPOSAL.md

TASK005_CANONICAL_GATE: ACCEPTED
TASK005_SPECIFICATION_VERSION: 1.1.18
TASK005_LIFECYCLE: DONE
TASK005_IMPLEMENTATION_AUTHORITY: NONE
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md

TASK006_CANONICAL_GATE: ACCEPTED
TASK006_SPECIFICATION_VERSION: 1.1.22
TASK006_LIFECYCLE: DONE
TASK006_IMPLEMENTATION_AUTHORITY: NONE
TASK006_PROPOSAL: docs/proposals/TASK-006-GATE-PROPOSAL.md

TASK007_CANONICAL_GATE: ACCEPTED
TASK007_SPECIFICATION_VERSION: 1.1.25
TASK007_LIFECYCLE: IN_PROGRESS
TASK007_IMPLEMENTATION_AUTHORITY: TASK_007_ONLY
TASK007_PROPOSAL: docs/proposals/TASK-007-GATE-PROPOSAL.md

TASK003_AC_OWNERSHIP_CONFLICT: ACCEPTED
TASK003_AC_028_CONTRIBUTORS: TASK-003; TASK-007
TASK003_AC_028_TERMINAL_OWNER: TASK-013
TASK003_AC_029_CONTRIBUTORS: TASK-003; TASK-013; TASK-016; TASK-022
TASK003_AC_029_TASK013_BRANCHES: PLUGIN_GRANT; AUDIT_EXPORT; MANUAL_MIGRATION_ADMIN
TASK003_AC_029_TASK016_BRANCHES: CREDENTIAL
TASK003_AC_029_TASK022_BRANCHES: PURGE
TASK003_AC_029_TERMINAL_OWNER: TASK-023

## 3. Phase 0 — intake, evidence and decision gate

Status: `DONE` for the foundation scope required by TASK-001..TASK-005.

Required outputs:

1. `PROJECT_INTAKE_REPORT.md` with repository/toolchain/host filesystem facts and discrepancy classifications: `DONE` and refreshed after the foundation toolchain/platform decisions; refresh again when those facts change.
2. Accepted `OQ-003` decision: `DONE` in ADR-0003—Rust/MSRV 1.98.0 and bundled SQLite 3.53.4 with source/options/checksum.
3. Accepted incremental `OQ-006` caps: `DONE` for TASK-002..TASK-005 in ADR-0005; later caps close before their consuming tasks.
4. Accepted target platform and ordinary Client peer-auth contract: `DONE` in ADR-0004; Admin features explicitly remain disabled until a later OQ-010 ADR.
5. A doc traceability check covering the normative namespaces and canonical-definition/range rules in Specification §0.5. Current documentation correction defines TASK-001 IDs; `TEST-DOC-001` makes the check reproducible during TASK-001 and requires PASS before that task is `DONE`.
6. `IMPLEMENTATION_REVIEW.md` updated so the foundation decisions needed to assess TASK-001..TASK-005 are recorded: `DONE`; TASK-004's accepted contract closes its gate findings, while each later task still requires its own start-gate audit.

Completion condition: the first task's full dependency set is accepted and there are no unrecorded conflicts. Phase 0 does not choose Provider, secret store or sandbox prematurely unless evidence is available.

## 4. Task start and completion contract

Before starting any task, Codex MUST answer from the documents:

```text
Feature/Requirement/AC/TEST IDs; exact files and dependency direction;
trusted and untrusted inputs; authenticated principal and authorization;
persisted state/transaction/effect boundaries; timeout/retry/idempotency;
positive, negative, concurrency, abuse and failure tests; completion evidence.
```

If any answer affects correctness/security/data integrity and is absent, classify and record it; do not infer it. Every external/process/file side effect requires a durable intent/recovery answer before implementation.

At task start, stable AC/TEST obligations must already be defined and copied into the task-start record; their repository commands may be created by that task. At task completion, every referenced TEST ID must map to a deterministic repository command/target/check and have recorded PASS evidence. This staged contract prevents both prose-only completion and a bootstrap circular dependency.

## 5. Task traceability and order

Detailed task bodies are normative in Specification §18. This table adds the required living status, traceability, file scope, security implications and do-not-change constraints.

| Task | Status | Feature / Requirements | Dependencies / decision gates | Likely files | Acceptance and tests | Security implications / Do not change |
|---|---|---|---|---|---|---|
| `TASK-001` Repository bootstrap | `DONE` | FUNC-001; SEC-020, DATA-006 | Phase 0 and OQ-003 accepted | root Cargo/toolchain/deny config, crate skeletons | AC-050, AC-051, AC-052, AC-053, AC-054; TEST-BOOT-001, TEST-BOOT-002, TEST-ARCH-001, TEST-NAME-001, TEST-SUPPLY-001, TEST-DOC-001 | Pin fixed SQLite path; no historical names or relaxed dependency edges |
| `TASK-002` Core values/error baseline | `DONE` | FUNC-001; REQ-001, API-010, DATA-012; SEC-017, SEC-020 | TASK-001; BASE-011, BASE-013, BASE-014; ADR-0003, ADR-0005; accepted REVIEW-GAP-003 | workspace deps/lock, `mengxia-types`, domain/errors, TASK-002 tests/docs | AC-055, AC-056, AC-057, AC-058, AC-059; TEST-TYPE-001, TEST-PARSE-001, TEST-TIME-001, TEST-ERROR-001, TEST-ARCH-002, TEST-SUPPLY-002, TEST-DOC-002 | Exact accepted codecs/fallible generator/errors/deps only; no Provider/Plugin/proto/Serde/DB/storage behavior or raw input/secret retention |
| `TASK-003` IPC, framing, Client identity | `DONE` | FUNC-001; API-001, API-002, API-003, API-008, API-009, API-010; SEC-005, SEC-013, SEC-014, SEC-017, SEC-020, SEC-021; REL-001, REL-006; CFG-001, CFG-003 | accepted supplement/start record; TASK-002; TASK-004; ADR-0004; ADR-0005 | exact supplement §4 scope | AC-060, AC-061, AC-062, AC-063, AC-064; eleven stable TASK-003 tests | Actor server-derived; Admin/product operations/TCP disabled; bounded IPC consumes opaque opened Library authority only |
| `TASK-004` SQLite/migration engine | `DONE` | FUNC-001; DATA-001, DATA-005, DATA-006, DATA-007, DATA-011; REL-001; SEC-017, SEC-020, SEC-021; CFG-001, CFG-003 | TASK-002; BASE-011, BASE-013, BASE-014, BASE-015, BASE-017; DEC-017, DEC-020, DEC-021, DEC-022; ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006; accepted implementation supplement | exact supplement §8 scope | AC-065, AC-066, AC-067, AC-068, AC-069, AC-070, AC-071, AC-072, AC-073; fourteen stable tests | Bootstrap lifecycle only; exact bounded scope; no TASK-003/Admin/later schema/capability |
| `TASK-005` BlobStorage/CAS primitives | `DONE` | FUNC-002 storage precondition; DATA-002, DATA-003, DATA-004, DATA-013; PERF-001; REL-001, REL-004, REL-006; SEC-017, SEC-020, SEC-021; CFG-001, CFG-003 | TASK-002, TASK-004; BASE-009, BASE-011, BASE-013..BASE-018; ADR-0002..ADR-0007; accepted supplement and start/completion records | exact supplement §3.1 narrow files/symbols | AC-074..AC-081; seventeen stable TASK-005 TEST IDs; local and reviewed formal gates PASS | Opaque source/root authority, atomic capacity, exact-case durable CAS and joined cleanup; no source deletion, DB/domain registration, product API or GC; TASK-006+ remain unauthorized |
| `TASK-006` Asset domain/persistence | `DONE` | FUNC-002, FUNC-003; REQ-001, REQ-002, REQ-004, REQ-005, REQ-008, REQ-011, REQ-012; DATA-001, DATA-007, DATA-009, DATA-010, DATA-011, DATA-013; SEC-017, SEC-020, SEC-021; REL-001, REL-004, REL-005, REL-006 | TASK-004, TASK-005; ADR-0008; accepted supplement/start/completion records | proposal §3 exact domain/app/ports/events/store/migration scope and immutable `0001_library_assets` | AC-082, AC-083, AC-084, AC-085, AC-086, AC-087, AC-088, AC-089, AC-090; fourteen TASK-006 TEST IDs; reviewed run `33257331689` PASS | No migration rewrite after apply; Blob dedup never merges Asset; no TASK-007 transport/CAS orchestration |
| `TASK-007` copy-only ingest slice | `IN_PROGRESS` | FUNC-002; REQ-001, REQ-002, REQ-008, REQ-010, REQ-011, REQ-013; DATA-002, DATA-003, DATA-004, DATA-009, DATA-013; API-001, API-002, API-003, API-008, API-010; SEC-005, SEC-013, SEC-017, SEC-020, SEC-021; REL-001, REL-004, REL-005, REL-006; PERF-001; CFG-001, CFG-003 | TASK-003, TASK-005, TASK-006; ADR-0002, ADR-0004, ADR-0005, ADR-0007, ADR-0008, ADR-0009; accepted supplement/start record | proposal §3 exact app/proto/CLI/daemon/config/platform/store/test/docs scope | AC-001..AC-009; nineteen stable TASK-007 TEST IDs; developer/formal gates | Copy only; fatal store gate preserved; reject adopt/reference; physical durability before registration; changed backend fails closed without rebind; no migration/TASK-008+ |
| `TASK-008` verify/recovery | `PENDING` | FUNC-001, FUNC-010; API-011; REL-004, REL-008; OPS-004 | TASK-007 | daemon/app/store/storage/CLI | corruption matrix, bounded pagination/cursor tests, provider-offline restart AC-015, startup cost | Deep verify explicit; unrelated local work allowed in degraded mode |
| `TASK-009` Project/Work/Take | `PENDING` | FUNC-004; REQ-003, REQ-004, REQ-006, REQ-007, REQ-012, REQ-014; SEC-014 | TASK-006 | domain/app/store/proto, `0002_projects_work` | AC-010, AC-011, AC-016; transition/concurrency/cross-Project tests | Project not tenant/Asset owner; no generic CRUD/direct state assignment |
| `TASK-010` Plugin package/Manifest | `BLOCKED` | FUNC-006; SEC-003, SEC-009, SEC-010, SEC-016, SEC-020 | TASK-001, TASK-002; OQ-010 before install/approve/activate/revoke | package/security/schema, `0003_plugin_packages` | AC-027; schema/tamper/publisher spoof/dependency tests | VERIFIED does not authenticate publisher; exact digest grant only |
| `TASK-011` Plugin protocol/hostile fixture | `BLOCKED` | FUNC-006, FUNC-007; API-004; REL-001, REL-006; SEC-017, SEC-021 | TASK-003, TASK-010; frame/log/process caps | plugin proto/framing/host/testkit | malformed/flood/crash/timeout/queue cap suite | Private channel only; bounded stdout/stderr/frames; no Core/Admin handle |
| `TASK-012` exact OS sandbox | `BLOCKED` | FUNC-007; SEC-001, SEC-002, SEC-005, SEC-021 | TASK-011; OQ-001, OQ-002; resource caps | platform sandbox/host/security tests | AC-020..AC-023 + mandatory real hostile suite | All required dimensions ENFORCED or deny; no backend-name/self-report shortcut |
| `TASK-013` Lease/Asset Broker/audit | `BLOCKED` | FUNC-006, FUNC-007, FUNC-010; SEC-004, SEC-005, SEC-006, SEC-008, SEC-019; DATA-011; OPS-001, OPS-002, OPS-003 | TASK-007, TASK-009, TASK-012; OQ-010 | security/host/brokers/store, `0004_plugin_security`; narrow privileged-dispatch denial for Plugin grant, audit export and manual migration Admin | AC-024, AC-026, AC-028; caller/race/revoke/audit/log-redaction/metric-schema tests | Caller/channel/run/digest binding; CAS path hidden; untrusted content grants no authority; ordinary Client cannot grant |
| `TASK-014` controlled FFmpeg Plugin | `PENDING` | FUNC-005, FUNC-007; SEC-009, SEC-017, SEC-020; REL-006 | TASK-013; accepted executable digest/resource caps | plugin/tool + contracts | timeout/cancel/malformed media/digest/output verify | argv only, no shell/PATH/DB/CAS; output untrusted until verified |
| `TASK-015` Recipe/Run runtime | `PENDING` | FUNC-005; REQ-006, REQ-009; API-005, API-006, API-007; DATA-010; REL-002, REL-003, REL-004, REL-005, REL-006, REL-007, REL-008 | TASK-009, TASK-014; job/queue/deadline caps | runtime/domain/app/store, `0005_runtime` | AC-012..AC-016, AC-031; DAG/attempt/crash/partial-success tests | Persist intent before effect; UNKNOWN no blind retry; retry creates Attempt |
| `TASK-016` Secret/Network Brokers | `BLOCKED` | FUNC-007, FUNC-008; SEC-006, SEC-007, SEC-011, SEC-012, SEC-015, SEC-017, SEC-021; CFG-002 | TASK-013, TASK-015; OQ-004, OQ-010; egress/cost/size caps | brokers/security/config | AC-023, AC-025, AC-044; SSRF/rebinding/redirect/canary rotation tests | No generic proxy/raw static secret to sandbox; no real egress before pass |
| `TASK-017` Provider selection gate | `BLOCKED` | FUNC-008, FUNC-012; API-005, API-006, API-007 | TASK-016; closes OQ-005 through its accepted Provider-selection ADRs | ADRs, adapter README/contracts | current official source/auth/state/idempotency/webhook evidence | No implementation from memory; no generic webhook listener; current blocker is TASK-016, not the decision output owned here; OQ-005 remains closed-before-implementation evidence for TASK-018..TASK-020 |
| `TASK-018` CLI Provider | `PENDING` | FUNC-008, FUNC-012; API-005, API-006, API-007; REL-002, REL-003, REL-006, REL-007 | TASK-017 | selected plugin/adapter | fake CLI + opt-in real contract, kill/timeout/env tests | verified executable/env; durable external ID; no shell |
| `TASK-019` HTTP Provider | `PENDING` | FUNC-008, FUNC-012; SEC-011, SEC-015, SEC-017; REL-002, REL-003, REL-006, REL-007 | TASK-017 | selected adapter/brokers | mock server + optional real; retry/rate/redirect/webhook-if-enabled | Broker-only egress; bounded streaming; adapter-specific callback contract |
| `TASK-020` Local/Hybrid/interoperability | `PENDING` | FUNC-012; G-001, G-003, G-008; DATA-008 | TASK-017, TASK-019 | adapter/integration/export | AC-030; reopen without plugin; relationship/resolve/register contracts | No Core schema change or provider type leakage |
| `TASK-021` Rights/classification/clearance | `BLOCKED` | FUNC-009; G-009, REQ-015, DATA-012; SEC-014, SEC-017, SEC-019 | TASK-009, TASK-013; OQ-009 | rights/domain/app/proto/store, `0006_rights_classification` | AC-040; correction/conflict/egress/cross-Project/audit E2E | UNKNOWN/CONFLICTED never implicit ALLOW; Provenance != Rights |
| `TASK-022` retention/GC/Purge | `BLOCKED` | FUNC-011; REQ-015; DATA-010, DATA-011; SEC-018, SEC-019 | TASK-008, TASK-013, TASK-021; OQ-008 | admin/app/storage/store/CLI | AC-041..AC-043; preview/hold/last-copy/concurrency/crash tests | Purge disabled until policy; exact target set; no retirement→delete inference |
| `TASK-023` release gate | `BLOCKED` | all enabled FUNC and P0 requirements; PERF-002 | enabled tasks complete; OQ-006 release SLO part | CI/docs/evidence/ops | AC-029; all mandatory suites, upgrades, PERF-002 benchmark report, fresh advisories | No silent skip/fabricated SLO/unsupported security claim |

### TASK-001 start record — 2026-08-21

- Scope: `TASK-001` only; repository bootstrap, empty crate/binary skeletons and its deterministic verification gates. No TASK-002 domain behavior or later feature is authorized.
- Feature/Requirements: `FUNC-001`; `SEC-020`; `DATA-006` (the TASK-001 obligation is limited to preserving the accepted bundled SQLite pin/boundary; runtime integration remains owned by TASK-004).
- Decisions/gates read: `OQ-003`; `DEC-001`; `DEC-007`; `DEC-019`; `DEC-020`; `BASE-001`; `BASE-011`; `BASE-014`; `BASE-016`; `ADR-0003`.
- Acceptance obligations: `AC-050`; `AC-051`; `AC-052`; `AC-053`; `AC-054`.
- Verification obligations: `TEST-BOOT-001`; `TEST-BOOT-002`; `TEST-ARCH-001`; `TEST-NAME-001`; `TEST-SUPPLY-001`; `TEST-DOC-001`.
- Planned file scope: root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`, `.gitignore`; `.github/workflows/ci.yml`; `scripts/verify-task-001.sh`, `scripts/check-supply-chain.sh`; empty manifests/targets under `crates/` and `bins/`; TASK-001-only verification code/fixtures in `crates/mengxia-testkit/`; this living task record.
- Dependency direction: `domain/types/events -> ports -> application -> infrastructure adapters -> daemon/CLI composition roots`; forbidden edges remain exactly Specification §5.3 and are enforced against Cargo metadata plus a negative fixture.
- Trust/security answers: Cargo manifests, lockfiles, repository paths, Git inventory, advisory data and tool output are validation inputs and are not trusted merely because parsing succeeds. TASK-001 introduces no authenticated principal, authorization decision, tenant context, secret, persistence transaction, network/service runtime, retry, idempotent command or destructive behavior. External advisory availability is bounded by the verification process and MUST fail as `UNVERIFIABLE`, never PASS.
- Test/evidence plan: positive workspace/metadata/build/fmt/Clippy/test checks; forbidden-edge negative fixture; canonical-name and tracked-file hygiene checks; fail-closed locked source/license/advisory check; stable-ID positive and negative parser checks. Concurrency, runtime retry/idempotency, authentication and tenant tests are not applicable to an empty repository skeleton.
- Completion evidence required: every listed TEST ID maps to a deterministic command and passes; every listed AC is checked against command/diff evidence; `SEC-020` passes; no new regression, unresolved applicable blocker, scope expansion or architecture drift remains.

### TASK-001 completion record — 2026-08-21

- Scope result: only repository bootstrap, empty canonical crate/binary skeletons, CI/policy configuration and TASK-001 verification infrastructure were added. No TASK-002 behavior or later capability was implemented.
- Commit/worktree evidence: the repository baseline change containing this record is limited to the reviewed code/document/config candidate inventory; ignored `.DS_Store` and Cargo `target/` content is excluded, and the tracked worktree must be clean when the commit is handed off.
- `TEST-BOOT-001`: `PASS` — `scripts/verify-task-001.sh TEST-BOOT-001`; arm64 host, Rust/Cargo 1.98.0 and locked metadata verified.
- `TEST-BOOT-002`: `PASS` — `scripts/verify-task-001.sh TEST-BOOT-002`; format, build, check, Clippy with warnings denied and all workspace tests passed for all targets/features.
- `TEST-ARCH-001`: `PASS` — `scripts/verify-task-001.sh TEST-ARCH-001`; allowed internal graph passed, the committed `domain -> application` fixture and explicit forbidden infrastructure edges were rejected, and pure crates explicitly forbid unsafe code.
- `TEST-NAME-001`: `PASS` — `scripts/verify-task-001.sh TEST-NAME-001`; the 17-package canonical inventory, approved code/document/config path allowlist, candidate tracked-file hygiene and explicit nested-build/editor/log/coverage/environment ignore matrix passed; ignored pre-existing `.DS_Store` and Cargo `target/` files remain untracked.
- `TEST-SUPPLY-001`: `PASS` — `scripts/verify-task-001.sh TEST-SUPPLY-001`; cargo-deny 0.20.2, lock/source/license/bans/current RustSec checks passed; the deterministic unavailable simulation returned `UNVERIFIABLE` with exit status 2.
- `TEST-DOC-001`: `PASS` — `scripts/verify-task-001.sh TEST-DOC-001`; canonical definitions/references/ranges/task lifecycle/dependencies passed, while unknown/duplicate/malformed/unmet negative cases were rejected.
- `AC-050`: `PASS` — pinned toolchain, locked complete metadata and all-target build/check/test evidence from `TEST-BOOT-001` and `TEST-BOOT-002`.
- `AC-051`: `PASS` — metadata direction, committed negative fixture, explicit forbidden-edge cases and pure-crate unsafe prohibition evidence from `TEST-ARCH-001`.
- `AC-052`: `PASS` — canonical package/binary/path inventory, Git candidate-inventory hygiene and required `.gitignore` coverage evidence from `TEST-NAME-001`.
- `AC-053`: `PASS` — exact internal dependency versions, Cargo.lock, fail-closed deny policy, fresh advisory fetch and explicit unavailable result from `TEST-SUPPLY-001`.
- `AC-054`: `PASS` — deterministic closed stable-ID registry, complete TASK-001 record and negative traceability cases from `TEST-DOC-001`.
- Security result — `SEC-020`: `PASS`; no third-party production dependency was added, CI actions/tool versions are exact-pinned, and advisory unavailability cannot be reported as clean. Runtime SQLite hardening under `DATA-006` remains correctly deferred to TASK-004; TASK-001 preserves Rust 1.98.0 and SQLite 3.53.4 decision traceability.
- Baseline/diff result: `BASELINE-003` records and resolves the stale pre-TASK-001 Current State descriptions; no new regression remains, and review found no secret, debug bypass, public API behavior, migration, architecture drift or later-task implementation.

### TASK-002 start record — 2026-08-21

- Scope: `TASK-002` only; typed UUIDv7 IDs, SHA-256 digest value, UTC timestamp, RevisionNo and the minimal safe typed error baseline. No TASK-003 or later behavior is authorized.
- Feature/Requirements: `FUNC-001`; `REQ-001`; `API-010`; `DATA-012`; `SEC-017`; `SEC-020`.
- Decisions/gates read: `BASE-011`; `BASE-013`; `BASE-014`; `ADR-0003`; `ADR-0005`; `REVIEW-GAP-003`; completed `TASK-001` evidence.
- Acceptance obligations: `AC-055`; `AC-056`; `AC-057`; `AC-058`; `AC-059`.
- Verification obligations: `TEST-TYPE-001`; `TEST-PARSE-001`; `TEST-TIME-001`; `TEST-ERROR-001`; `TEST-ARCH-002`; `TEST-SUPPLY-002`; `TEST-DOC-002`.
- Planned file scope: root/workspace Cargo dependency declarations and lockfile; `crates/mengxia-types`; the minimal error module in `crates/mengxia-domain`; TASK-002-only tests/verification wiring; synchronized canonical/current-state lifecycle records. No proto, schema, migration, persistence, storage, Provider, Plugin, IPC or binary behavior.
- Public contract: exact Specification §8.1.1 and §14 contracts incorporated from the user-accepted `docs/proposals/TASK-002-GATE-PROPOSAL.md`.
- Security answers: all textual values are untrusted, exactly bounded, strictly parsed and canonicalized; error values retain no raw rejected input. This task has no authenticated operation, authorization decision, tenant, secret store, persistence transaction, network/process/file side effect, retry, idempotent command, migration or destructive behavior. ID generation is concurrency-tested and uses direct fallible OS time/entropy with the stateless UUID builder; it uses no MengXia-owned or dependency-owned shared counter/generator.
- Dependency answer: exact `uuid` 1.24.1/std, `getrandom` 0.4.3/std, `time` 0.3.55/std+formatting+parsing and dev-only `proptest` 1.11.0/std pins/features are accepted with default features disabled and must pass the existing fail-closed lock/license/advisory policy.
- Completion evidence required: every listed TEST ID maps to a deterministic command and passes; every listed AC and applicable security requirement has evidence; TASK-001 baseline remains green; no new regression, scope expansion, public API drift or unresolved applicable blocker remains.

### TASK-002 completion record — 2026-08-21

- Commit/worktree state: branch `main`, parent baseline commit `60b344571874946dc7fb77936ca1d50f40cb045d`; this record is part of the reviewed TASK-002 candidate and therefore does not self-claim its resulting commit hash. The completion handoff must report that hash and a clean tracked worktree.
- `TEST-TYPE-001`: `PASS` — `scripts/verify-task-002.sh TEST-TYPE-001`; UUIDv7 generation, marker typing, trait surface, byte/text round trips, deterministic time/entropy failure seams and parallel uniqueness sample passed.
- `TEST-PARSE-001`: `PASS` — `scripts/verify-task-002.sh TEST-PARSE-001`; malformed, noncanonical, non-ASCII, wrong UUID version/variant and numeric/time boundary inputs were rejected.
- `TEST-TIME-001`: `PASS` — `scripts/verify-task-002.sh TEST-TIME-001`; UTC year/nanosecond bounds, unique canonical `Z` text and checked revision exhaustion passed.
- `TEST-ERROR-001`: `PASS` — `scripts/verify-task-002.sh TEST-ERROR-001`; all 25 stable codes round-trip exactly and typed value/domain errors expose only static safe diagnostics.
- `TEST-ARCH-002`: `PASS` — `scripts/verify-task-002.sh TEST-ARCH-002`; exact public dependency surface, forbidden production edges and marker-mismatch compile-fail fixture passed.
- `TEST-SUPPLY-002`: `PASS` — `scripts/verify-task-002.sh TEST-SUPPLY-002`; exact pins/features/lock/license/advisory graph passed, production resolves only `getrandom` 0.4.3, dev-only `proptest` resolves 0.3.4, and unavailable advisory data fails as `UNVERIFIABLE`.
- `TEST-DOC-002`: `PASS` — `scripts/verify-task-002.sh TEST-DOC-002`; stable IDs, start/completion records and synchronized current-state markers passed positive and stale-state negative checks.
- `AC-055`: `PASS` — `TEST-TYPE-001`, `TEST-PARSE-001` and `TEST-ARCH-002` prove opaque marker-safe UUIDv7 values, canonical codecs and fallible stateless generation.
- `AC-056`: `PASS` — `TEST-TYPE-001` and `TEST-PARSE-001` prove exact 32-byte/lowercase-hex digest behavior without hashing APIs.
- `AC-057`: `PASS` — `TEST-TIME-001` and `TEST-PARSE-001` prove bounded UTC timestamps, unique canonical text and checked revisions.
- `AC-058`: `PASS` — `TEST-ERROR-001` proves the exact stable error taxonomy, typed mappings and input-safe diagnostics.
- `AC-059`: `PASS` — `TEST-ARCH-002` and `TEST-SUPPLY-002` prove the accepted public/dependency boundary and fail-closed supply policy.
- `SEC-017`: `PASS` — strict bounded parsers reject ambiguous/noncanonical/untrusted input and errors retain no rejected input; `TEST-PARSE-001` and `TEST-ERROR-001` provide negative evidence.
- `SEC-020`: `PASS` — exact dependency pins/features, lockfile, license/source/bans/advisory checks and the auditable dev-only duplicate path pass `TEST-SUPPLY-002` and `TEST-ARCH-002`.
- Baseline comparison: `scripts/verify-task-001.sh` passed after TASK-002, including format/build/check/Clippy/all-workspace tests, architecture/naming/supply/document gates. No pre-existing failure was reclassified and no new regression remains.
- Diff/scope result: only accepted TASK-002 values/errors, exact dependency policy, fixtures/tests/scripts and synchronized lifecycle documents changed. No proto, Serde, schema, migration, persistence, storage, IPC, binary, Provider, Plugin, authentication/authorization or destructive behavior was added; no test was deleted or weakened.
- Unexecuted required tests: none. TASK-003 and later tasks remain unauthorized and were not started.

### Accepted TASK-004-before-TASK-003 sequencing — 2026-08-21

- Decision: user selected Option A from `docs/proposals/TASK-003-GATE-PROPOSAL.md`.
- Reason: TASK-003 must compare peer UID with the durable recorded Library owner, while TASK-004 owns first-create owner persistence and the Library lock.
- Boundary: TASK-004 produces an opened Library context containing durable owner/lock authority; TASK-003 consumes that context only. Proto/framing/IPC crates cannot depend on SQLite or infer owner authority from eUID, environment, CLI or request fields.
- Stable-ID guarantee: no Task ID is renumbered or split. This is an execution dependency correction, not a Feature/Requirement/AC reassignment.
- Downstream invariants: TASK-006 continues to depend on TASK-004 and TASK-005; TASK-007 continues to depend on TASK-003 and TASK-006; TASK-011 continues to depend on TASK-003 and TASK-010. The dependency graph must remain acyclic.
- Completion: TASK-004 is `DONE` under Specification v1.1.13, ADR-0006, the accepted supplement, the exact start record and the completion evidence below. TASK-003 remains `PENDING` and unauthorized until its own stable registry/start gate is reviewed; no later behavior was started.

### TASK-003 start record — 2026-08-25

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

### TASK-003 completion record — 2026-08-26

- Evidence commit: `4f7bf27855b05c5080790aae3221ee10ae662431`; reviewed GitHub Actions run `32914222948` completed `SUCCESS` for both `Rust 1.98 arm64 macOS` and `TASK-003 real second-UID authorization` on the required `macos-26` runner. The exact toolchain preflight, retained TASK-001/TASK-002/TASK-004 baselines, all ten unprivileged mappings and the privileged mapping passed.
- Local developer evidence: `./scripts/verify-task-003.sh` passed after the final test-fixture correction. The real second-UID branch remained correctly excluded from local PASS ownership and was executed only by the formal wrapper.
- Diff result: implementation remains inside accepted §4. No product operation, Admin capability, persistence write, migration 0001+, CAS, TCP/HTTP, Provider/Plugin behavior, secret, debug bypass, dependency expansion or public authority leak was introduced. Required unexecuted tests: none.

`AC-060`: `PASS`; EVIDENCE: TEST-FRAME-001
`AC-061`: `PASS`; EVIDENCE: TEST-PROTO-001+TEST-HANDSHAKE-001
`AC-062`: `PASS`; EVIDENCE: TEST-IPC-MACOS-001
`AC-063`: `PASS`; EVIDENCE: TEST-AUTH-001+TEST-CLI-001+TEST-ARCH-003
`AC-064`: `PASS`; EVIDENCE: TEST-HANDSHAKE-001+TEST-ENDPOINT-003+TEST-CONFIG-003

`TEST-PROTO-001`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-PROTO-001
`TEST-FRAME-001`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-FRAME-001
`TEST-HANDSHAKE-001`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-HANDSHAKE-001
`TEST-IPC-MACOS-001`: `PASS`; EVIDENCE: scripts/verify-task-003-formal-second-uid.sh#TEST-IPC-MACOS-001
`TEST-ENDPOINT-003`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-ENDPOINT-003
`TEST-CONFIG-003`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-CONFIG-003
`TEST-AUTH-001`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-AUTH-001
`TEST-CLI-001`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-CLI-001
`TEST-ARCH-003`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-ARCH-003
`TEST-SUPPLY-003`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-SUPPLY-003
`TEST-DOC-003`: `PASS`; EVIDENCE: scripts/verify-task-003.sh#TEST-DOC-003

- `SEC-005`: `PASS` — peer identity is derived from the accepted Unix channel before any frame is read; caller actor/Admin fields cannot override it.
- `SEC-013`: `PASS` — malformed framing/proto/depth/version inputs fail closed with bounded work and safe errors.
- `SEC-014`: `PASS` — ordinary Client authorization is bound to the durable opened-Library owner UID; Admin remains disabled.
- `SEC-017`: `PASS` — configuration, endpoint and frame inputs are typed and bounded before mutation or decode.
- `SEC-020`: `PASS` — locked/offline dependency, descriptor, generator and supply-chain checks pass without ambient code generation.
- `SEC-021`: `PASS` — endpoint ownership, ACL, marker, collision and stale-cleanup matrices preserve fail-closed authority.

FORMAL_SECOND_UID_CI_REPOSITORY: XiaTian-X/MengXia
FORMAL_SECOND_UID_CI_WORKFLOW: .github/workflows/ci.yml
FORMAL_SECOND_UID_CI_JOB: task-003-second-uid
FORMAL_SECOND_UID_CI_RUNNER: macos-26
FORMAL_SECOND_UID_CI_COMMIT: 4f7bf27855b05c5080790aae3221ee10ae662431
FORMAL_SECOND_UID_CI_RUN: 32914222948
FORMAL_SECOND_UID_CI_RESULT: PASS

### TASK-004 start record — 2026-08-22

```text
TASK: TASK-004
STATUS: IN_PROGRESS
DEPENDENCIES: TASK-002; BASE-011; BASE-013; BASE-014; BASE-015; BASE-017;
  DEC-017; DEC-020; DEC-021; DEC-022; ADR-0001; ADR-0003; ADR-0004;
  ADR-0005; ADR-0006
REQUIREMENTS: FUNC-001; DATA-001; DATA-005; DATA-006; DATA-007; DATA-011;
  REL-001; SEC-017; SEC-020; SEC-021; CFG-001; CFG-003
ACCEPTANCE: AC-065; AC-066; AC-067; AC-068; AC-069; AC-070; AC-071;
  AC-072; AC-073
TESTS: TEST-SQLITE-004; TEST-CONFIG-004; TEST-BOOTSTRAP-004; TEST-PATH-004;
  TEST-MIGRATION-004; TEST-LOCK-004; TEST-QUEUE-004; TEST-ERROR-004;
  TEST-RECOVERY-004; TEST-WAL-004; TEST-CORRUPTION-004; TEST-ARCH-004;
  TEST-SUPPLY-004; TEST-DOC-004
AUTHORIZED: Cargo.toml; Cargo.lock; narrow workspace/naming/supply policy metadata;
  .github/workflows/ci.yml for the exact attested macos-26 gate only;
  crates/mengxia-store-sqlite/**; crates/mengxia-platform-fs/**;
  docs/provenance/macos-acl-ffi-toolchain-v1.toml;
  migrations/sqlite/0000_store_bootstrap.sql;
  third_party/libsqlite3-sys-0.38.2/**; TASK-004 tests/scripts/fixtures;
  synchronized canonical documents; the canonical error registry/module only for
  STORAGE_BUSY and STORAGE_CONFIGURATION_ERROR
FORBIDDEN: TASK-003 transport/daemon/CLI/Admin; migration 0001 or later;
  domain repositories; Blob/CAS, Provider, Plugin, Credential, Project, Rights,
  GC/Purge or product behavior; raw SQL API; custom VFS; system SQLite; unbounded
  wait/retry/detached work; architecture/public-interface expansion
```

- Accepted contract: `docs/proposals/TASK-004-GATE-PROPOSAL.md`, incorporated by
  Specification v1.1.13. The supplement's exact matrices control where this compact
  record summarizes behavior.
- Entry evidence: TASK-001/TASK-002 remain green; Option A/BASE-017 and ADR-0003,
  ADR-0004, ADR-0005, ADR-0006
  are accepted; AC-065..AC-073 and all fourteen TEST IDs are canonically defined;
  the developer/attested build distinction prevents local builds from claiming
  formal supply-chain PASS.
- Completion rule: do not mark DONE until each AC and TEST above has reproducible
  PASS evidence and the complete pre-existing workspace baseline remains green.
- Gate evidence update — 2026-08-24: `scripts/verify-task-004.sh` now maps all
  fourteen TEST IDs to executable commands, retains format/check/Clippy/full-workspace
  baselines, and is invoked by the reviewed arm64 `macos-26` workflow. The complete
  script passes locally in the default non-attested developer class, including
  environment rejection and compile/lint negative fixtures. Formal CI initially
  rejected the byte-distinct App Store-derived tool pins before Cargo; the corrected
  profile now uses reviewed runner-XIP clang/libtool bytes and emits observed digests
  before fail-closed comparison. Reviewed CI run `32695815747` passed the exact
  preflight, retained TASK-001 baseline and all fourteen TASK-004 gates.
- Code-review correction — 2026-08-24: the accepted §5.3 first-create clock/UUID
  production orchestration and `ID_GENERATION_UNAVAILABLE` mapping were absent even
  though the previous local gate passed (`REPO_STALE`). Fresh bootstrap now performs
  read-only parent authorization, samples each source exactly once before any root
  mutation, persists the one identity/timestamp through owner open and restart, and
  has absent/empty-root failure evidence for clock, timestamp and UUID source errors.
  `TEST-BOOTSTRAP-004` and `TEST-ERROR-004` include these cases; formal CI evidence
  passed in run `32695815747`.

### TASK-004 completion record — 2026-08-24

- Evidence commit: `bfcb15124cfc89e30656559e84a04c88be1561db`; reviewed GitHub Actions run `32695815747` completed `SUCCESS` on `macos-26-arm64` image `20260728.0273.1` in 4m07s. The exact attested Xcode/SDK/clang preflight, retained TASK-001 gates and complete TASK-004 gate script all passed.
- Local developer evidence: default non-attested `scripts/verify-task-004.sh TEST-SUPPLY-004` passed its tool/source/environment policy, advisories, bans, licenses, sources and retained format/check/Clippy/full-workspace test baseline without requiring runner-XIP tool bytes.
- `TEST-SQLITE-004`: `PASS`
- `TEST-CONFIG-004`: `PASS`
- `TEST-BOOTSTRAP-004`: `PASS`
- `TEST-PATH-004`: `PASS`
- `TEST-MIGRATION-004`: `PASS`
- `TEST-LOCK-004`: `PASS`
- `TEST-QUEUE-004`: `PASS`
- `TEST-ERROR-004`: `PASS`
- `TEST-RECOVERY-004`: `PASS`
- `TEST-WAL-004`: `PASS`
- `TEST-CORRUPTION-004`: `PASS`
- `TEST-ARCH-004`: `PASS`
- `TEST-SUPPLY-004`: `PASS`
- `TEST-DOC-004`: `PASS`
- `AC-065`: `PASS` — exact bundled SQLite and reviewed runner-XIP toolchain evidence passed before Cargo with no fallback.
- `AC-066`: `PASS` — path/bootstrap/lock gates prove fail-closed first-create authority and durable intent ordering.
- `AC-067`: `PASS` — migration/reopen/corruption gates prove the immutable bootstrap schema, checksum and typed singleton contract.
- `AC-068`: `PASS` — lock/recovery gates prove one retained Library authority, finite contention and no split-brain cleanup guess.
- `AC-069`: `PASS` — queue tests prove bounded admission, exact terminal disposition and joined shutdown.
- `AC-070`: `PASS` — architecture/supply gates prove bootstrap-only schema, isolated FFI, source-pinned SQLite and no raw/later capability.
- `AC-071`: `PASS` — SIGKILL, WAL/SHM and deterministic corruption matrices recover proven state or fail closed with redacted errors.
- `AC-072`: `PASS` — configuration tests prove pure typed tightening-only validation before mutation without production source precedence claims.
- `AC-073`: `PASS` — real/synthetic path and ACL matrices prove retained whole-prefix authority and isolated bounded FFI.
- `SEC-017`: `PASS` — typed/bounded input and path/config/schema validation fail closed before mutation; diagnostics retain no rejected input.
- `SEC-020`: `PASS` — exact source/tool/dependency pins, offline lock, advisory, bans, license and source gates pass in formal CI.
- `SEC-021`: `PASS` — owner/mode/ACL/no-follow/retained-descriptor and lock/recovery negative matrices pass without unauthorized cleanup.
- Baseline/diff result: no TASK-001 regression, public API expansion, architecture drift, migration rewrite, secret, debug bypass or TASK-003/later behavior was introduced. Required unexecuted tests: none.

### TASK-005 accepted gate / pre-start record — 2026-08-26

- Status: `PENDING / READY FOR START`; this is not an `IN_PROGRESS` record and grants
  no implementation authority.
- Accepted contract: Specification v1.1.18, ADR-0007 and
  `docs/proposals/TASK-005-GATE-PROPOSAL.md`.
- Stable acceptance: AC-074, AC-075, AC-076, AC-077, AC-078, AC-079, AC-080,
  AC-081.
- Stable tests: TEST-CONFIG-005, TEST-NAMESPACE-005, TEST-PATH-005,
  TEST-SOURCE-005, TEST-STREAM-005, TEST-CONTROL-005, TEST-RESOURCE-005,
  TEST-PROMOTE-005, TEST-LOCATION-005, TEST-RECOVERY-005, TEST-ORPHAN-005,
  TEST-CONCURRENCY-005, TEST-ERROR-005, TEST-LIFECYCLE-005, TEST-ARCH-005,
  TEST-SUPPLY-005, TEST-DOC-005.
- Activation condition: copy the proposal §16.1 exact start record into this Plan,
  switch canonical/AGENTS lifecycle to `IN_PROGRESS`, and run the retained document
  and TASK-003 aggregate gates once before production edits.
- Completed-task disposition: TASK-004 remains `DONE`; its current two-entry
  namespace is valid repository reality. The bounded optional-`storage`
  compatibility change is an authorized TASK-005 implementation obligation, not a
  retroactive TASK-004 failure. TASK-003 has no required production change.

### TASK-005 start record — 2026-08-26

STATUS: IN_PROGRESS
SCOPE: TASK-005 ONLY — Local BlobStorage/CAS custody primitive; no product ingest,
       database registration, domain Location persistence, deletion or GC.

FEATURE: FUNC-002 (storage precondition only; feature remains incomplete)
REQUIREMENTS:
  DATA-002, DATA-003, DATA-004, DATA-013, PERF-001, REL-001, REL-004, REL-006,
  SEC-017, SEC-020, SEC-021,
  CFG-001, CFG-003, BASE-009, BASE-011, BASE-013, BASE-014, BASE-015,
  BASE-016, BASE-017, BASE-018
DECISIONS:
  ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007
PREREQUISITES: TASK-002 DONE; TASK-004 DONE
ACCEPTANCE:
  AC-074, AC-075, AC-076, AC-077, AC-078, AC-079, AC-080, AC-081
TESTS:
  TEST-CONFIG-005, TEST-NAMESPACE-005, TEST-PATH-005, TEST-SOURCE-005,
  TEST-STREAM-005, TEST-CONTROL-005, TEST-RESOURCE-005, TEST-PROMOTE-005,
  TEST-LOCATION-005, TEST-RECOVERY-005, TEST-ORPHAN-005,
  TEST-CONCURRENCY-005, TEST-ERROR-005, TEST-LIFECYCLE-005, TEST-ARCH-005,
  TEST-SUPPLY-005, TEST-DOC-005
DEVELOPER_GATE: scripts/verify-task-005.sh developer (FAST_PASS only)
FORMAL_COMPLETION_GATE: scripts/verify-task-005.sh formal (all TEST IDs PASS)
AUTHORIZED_FILES: proposal §3.1 exact list and symbol restrictions
FORBIDDEN: proposal §3.2; TASK-006 and later remain unauthorized

### TASK-005 local completion candidate — 2026-08-27 (historical pre-CI state)

- Worktree evidence: the exact §3.1 implementation scope and tests are present but
  uncommitted. `scripts/verify-task-005.sh developer` passes after the final scope
  and lifecycle corrections.
- Full local evidence: `scripts/verify-task-005.sh formal` passes on the current APFS
  host, including all seventeen TEST IDs, 30 KILL points, 78 named fault seams,
  generated 1/10/100 GiB O(buffer) streams, supply-chain checks, retained
  TASK-001/TASK-002/TASK-004/TASK-003 gates and workspace-wide offline tests.
- Diff evidence: every changed path is inside proposal §3.1; the two CAS/recovery
  integration tests live under the authorized `mengxia-storage-local/tests/**`
  boundary. No TASK-006 schema/domain/registration, product API, delete/GC,
  Provider/Plugin/Admin behavior, secret, debug bypass or unbounded worker was added.
- Remaining evidence at that point: a committed candidate still had to pass the
  reviewed `macos-26` main CI job running `scripts/verify-task-005.sh formal`; until
  then TASK-005 correctly remained `IN_PROGRESS`. The completed evidence follows.

### TASK-005 completion record — 2026-08-27

- Scope result: TASK-005's Local BlobStorage/CAS custody primitive is implemented
  within proposal §3.1 only. No product ingest, database/domain registration,
  migration 0001+, source deletion, orphan deletion, GC/Purge, Admin, Plugin,
  Provider, Credential or later-task behavior was added.
- Evidence commit: `f516faafe50707b88f51f25c03be07f917f8943f` (implementation
  commit `88e7b3413db5607651f2c842f6d0c1f03d513968` plus the exact
  Clippy build-class gate correction); reviewed GitHub Actions run `33073580258`
  completed `SUCCESS` on `macos-26`.
- Gate correction evidence: run `33072816350` passed the seventeen fast TASK-005
  mappings, then correctly failed closed because the new gate retained attested FFI
  class around Cargo's legitimate Clippy workspace wrapper. Classification:
  `REPO_STALE`. The correction reused the established TASK-001/004 Clippy-only
  environment boundary, added an architecture regression assertion and did not
  weaken the build script or production behavior.
- `TEST-CONFIG-005`: `PASS`
- `TEST-NAMESPACE-005`: `PASS`
- `TEST-PATH-005`: `PASS`
- `TEST-SOURCE-005`: `PASS`
- `TEST-STREAM-005`: `PASS`
- `TEST-CONTROL-005`: `PASS`
- `TEST-RESOURCE-005`: `PASS`
- `TEST-PROMOTE-005`: `PASS`
- `TEST-LOCATION-005`: `PASS`
- `TEST-RECOVERY-005`: `PASS`
- `TEST-ORPHAN-005`: `PASS`
- `TEST-CONCURRENCY-005`: `PASS`
- `TEST-ERROR-005`: `PASS`
- `TEST-LIFECYCLE-005`: `PASS`
- `TEST-ARCH-005`: `PASS`
- `TEST-SUPPLY-005`: `PASS`
- `TEST-DOC-005`: `PASS`
- `AC-074`: `PASS` — source-free typed configuration, exact path headroom and
  config/authority binding fail before mutation.
- `AC-075`: `PASS` — Blob root, fixed Library binding, internal directories/files
  and local source authority are descriptor-first, no-follow, exact-case and bounded;
  no raw path/fd/lock/general opener escapes the infrastructure boundary.
- `AC-076`: `PASS` — the retained regular-file handle, before/after identity and one
  EOF probe detect source mutation; success, failure and crash never alter source
  bytes or metadata.
- `AC-077`: `PASS` — SHA-256 and exact length use one O(buffer) stream; every accepted
  cancellation/deadline checkpoint is joined, and formal 1/10/100 GiB evidence
  keeps media outside DB/protocol/logs.
- `AC-078`: `PASS` — one atomic logical/physical admission enforces all concurrency,
  size, staging and free-space limits without a second backpressure result or
  oversubscription.
- `AC-079`: `PASS` — exact-case no-replace promotion, rehash, sync ordering and
  verified dedup produce durable canonical bytes; backend-instance identity and the
  opaque 85-byte locator remain stable across same-inode root rename.
- `AC-080`: `PASS` — every named crash/fault prefix and bounded orphan/recovery path
  preserves or reports evidence, accounts safely and never guesses ownership or
  deletes prior-process staging automatically.
- `AC-081`: `PASS` — joined workers/channels, panic paths, shutdown and lock lifetime
  have one bounded terminal disposition with no detached work.
- `SEC-017`: `PASS` — untrusted config/path/source/metadata/digest inputs are typed,
  bounded and mapped to static redacted errors.
- `SEC-020`: `PASS` — locked offline build, exact attested toolchain, advisories,
  bans, licenses and sources passed reviewed CI.
- `SEC-021`: `PASS` — descriptor-first no-follow authority, owner/mode/ACL checks,
  exact-case no-clobber publish and fail-closed cleanup/recovery matrices pass.
- Formal detail: all 30 KILL points, 78 fault seams and generated 1/10/100 GiB
  O(buffer) streams passed; retained TASK-001/TASK-002/TASK-004/TASK-003 and full
  workspace gates passed. Required unexecuted tests: `NONE`.
- Diff review: no scope creep, architecture drift, accidental API expansion,
  migration rewrite, secret, debug bypass, unbounded retry/queue or backward-
  compatibility regression was found.
- Lifecycle result: TASK-005 is `DONE`; implementation authority is `NONE`;
  this remained true until TASK-006 received the independent start gate below.

### TASK-006 start record — 2026-08-28

STATUS: IN_PROGRESS
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

Activation evidence: reviewed proposal v0.2.1 closed all external findings; the
user accepted the candidate and authorized execution on 2026-08-28. The exact
migration bytes/hash and existing crate/lifecycle dependency boundaries were
revalidated, then document/naming/format and retained TASK-005 developer gates
passed before production edits. The accepted contract is proposal v0.2.2 and
Specification v1.1.22.

### TASK-006 completion record — 2026-08-29

- Scope result: implemented only the accepted Asset domain, typed DTO/row boundaries,
  CommandRecord/event persistence, immutable `0001_library_assets`, recovery and
  bounded single-writer lifecycle. No TASK-007 IPC/CLI/source/CAS orchestration,
  destructive behavior, later migration, dependency or unsafe/FFI expansion was added.
- Evidence commit: `60b6616c20d677632ca25b8b72340fc3a639db54`.
- Review correction commit: `10455605556984e48def16efc27fb52338109944`.
- Local command: `scripts/verify-task-006.sh formal` — `PASS` on the exact correction.
- External evidence: reviewed GitHub Actions run `33257331689` at
  `10455605556984e48def16efc27fb52338109944` — TASK-006 formal aggregate `PASS`
  in 12m05s; retained TASK-003 real second-UID job `PASS` in 6m48s.
- First-run disposition: run `33256714550` found a `REPO_STALE` retained TASK-005
  Blob-lock lifetime race caused by a close-on-exec descriptor inherited during a
  concurrent spawn. The private lock guard now explicitly unlocks on drop and a
  surviving-duplicate-descriptor regression passes; public API, migrations,
  dependencies and architecture boundaries are unchanged.
- `TEST-DOMAIN-006`: `PASS` — aggregate/value/state/cap invariants and marker
  separation, including boundary/property and compile-fail evidence.
- `TEST-MAPPER-006`: `PASS` — exact domain/row round trips and malformed-row classes.
- `TEST-MIGRATION-006`: `PASS` — fixed 0001 bytes/digest/order, 0000 preservation,
  upgrade, reopen and rollback behavior.
- `TEST-SCHEMA-006`: `PASS` — complete schema/table/index/FK/trigger/row allowlist
  and corruption negatives.
- `TEST-COMMAND-006`: `PASS` — command binding, external/pure claim, replay,
  rejection and recovery state matrix.
- `TEST-CONCURRENCY-006`: `PASS` — duplicate command, shared Blob, expected-revision
  and queue/shutdown races.
- `TEST-EVENT-006`: `PASS` — atomic state/outcome/event allocation, rollback,
  overflow and append-only denial.
- `TEST-CUSTODY-006`: `PASS` — consumed DurableBlob/single Member, shared
  Blob/distinct Asset and Location-only mutation assertions.
- `TEST-ERROR-006`: `PASS` — exact variant/code/retry/static-display mapping and
  redaction canaries.
- `TEST-RECOVERY-006`: `PASS` — migration/transaction SIGKILL and statement-fault
  prefixes have exact restart dispositions.
- `TEST-LIFECYCLE-006`: `PASS` — receipt drop, panic, backpressure, joined shutdown
  and Library-lock lifetime remain bounded.
- `TEST-ARCH-006`: `PASS` — dependency/public surface/file scope and forbidden
  fixtures reject unauthorized expansion.
- `TEST-SUPPLY-006`: `PASS` — locked/offline/minimal dependencies, advisories,
  licenses and sources pass.
- `TEST-DOC-006`: `PASS` — supplement/ADR/requirements/AC/TEST/lifecycle and
  downstream ownership agree.
- `AC-082`: `PASS` — typed aggregate/value/row boundaries enforce the accepted
  Asset graph invariants without exposing persistence representation.
- `AC-083`: `PASS` — immutable 0001 migration and complete reopen validation preserve
  0000 and reject incompatible prefixes/corruption.
- `AC-084`: `PASS` — managed registration consumes exact custody proof, preserves
  shared Blob versus distinct Asset identity and rejects invalid graph construction.
- `AC-085`: `PASS` — durable command claim, duplicate, completion, rejection and
  prior-runtime recovery produce the exact replay/conflict/fail-closed outcomes.
- `AC-086`: `PASS` — expected-revision mutation and append-only events are atomic,
  monotonic and conflict-safe.
- `AC-087`: `PASS` — state, command outcome, provenance and domain events commit or
  roll back together with allocator integrity.
- `AC-088`: `PASS` — persisted custody/location data remains opaque, bounded and
  separated from Asset identity and revision semantics.
- `AC-089`: `PASS` — writer admission, receipt loss, panic, shutdown and recovery
  retain one bounded joined disposition without detached work.
- `AC-090`: `PASS` — exact file/dependency/public-surface scope, supply policy,
  documentation and all retained repository gates pass with no regression.
- `SEC-017`: `PASS` — untrusted DTO/row/configuration inputs are typed, bounded and
  mapped to static redacted errors; malformed rows fail closed.
- `SEC-020`: `PASS` — locked offline build, pinned toolchain, advisories, bans,
  licenses and sources passed reviewed CI.
- `SEC-021`: `PASS` — opaque custody authority, transaction/recovery fail-closed
  behavior, immutable migrations and negative architecture tests pass.
- `SEC-005`: `NOT_APPLICABLE`; `SEC-013`: `NOT_APPLICABLE`; `SEC-014`:
  `NOT_APPLICABLE` — TASK-006 exposes no product transport/authentication boundary,
  and architecture tests prove no caller-supplied principal or tenant claim entered.
- Baseline comparison: all retained TASK-001/TASK-002/TASK-004/TASK-003/TASK-005,
  workspace, Clippy and supply-chain gates pass. No new regression remains.
- Required unexecuted tests: `NONE`.
- Diff review: no scope creep, accidental API expansion, architecture drift,
  migration rewrite, unnecessary dependency, unsafe expansion, secret/debug bypass,
  unbounded retry/queue or backward-compatibility regression remains.
- Lifecycle result: TASK-006 is `DONE`; implementation authority is `NONE`;
  TASK-007 and every later task remain unauthorized pending an independent gate.

### TASK-007 start record — 2026-08-30

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
AUTHORIZED_FILES: accepted proposal v0.1.4 §3 exact list and restrictions
FORBIDDEN: proposal §3.1; root rebind and TASK-008+ remain unauthorized
```

Activation evidence: independent review of proposal v0.1.2 found one existing-store
fatal-gate mismatch, an incomplete wire safe-message contract and an abbreviated
Decision list. Proposal v0.1.3 closed those three against the real TASK-003/005/006
interfaces; v0.1.4 additionally records the reviewed protocol-intent, endpoint
pre-publication recovery, retained-evidence and cross-kind raw-ID corrections without
changing migrations or later-task authority. The user authorized correction and
TASK-007-only implementation. Canonical document, naming, formatting and retained
baseline gates must pass before completion review.

Local completion-review checkpoint — 2026-08-31: the TASK-007 candidate, all
nineteen developer mappings, full workspace tests, Clippy, docs, naming and retained
developer aggregates pass after correcting `REVIEW-CONFLICT-016` through `018`.
Those corrections restore legacy handshake range/error fidelity, complete config
file post-read authority revalidation and retain the product-session permit through
terminal response close. Required unexecuted evidence is the reviewed formal
`macos-26` aggregate (including the separate real-second-UID job); therefore the
task remains `IN_PROGRESS` and implementation authority remains `TASK_007_ONLY`.

## 6. Phases and gates

| Phase | Tasks | Entry gate | Exit evidence |
|---|---|---|---|
| 0 Intake/decisions | documentation only | repository inspected | `DONE` for recorded foundation decisions; TASK-005's contract and completion evidence are accepted; later tasks retain their own gates |
| 1 Foundation authority/data | TASK-001..TASK-004 | Phase 0 complete | build + authenticated IPC + fixed/hardened SQLite evidence |
| 2 Managed custody | TASK-005..TASK-009 | Phase 1 | copy-ingest/recovery/domain E2E and crash evidence |
| 3 Plugin authority | TASK-010..TASK-013 | Admin/platform/caps decisions | hostile protocol + exact sandbox + lease/audit evidence |
| 4 Runtime/brokers | TASK-014..TASK-016 | Phase 3 | durable runtime, secret/egress/SSRF/canary evidence |
| 5 Provider/rights/destructive | TASK-017..TASK-022 | relevant OQ decisions | Provider portability, rights and destructive lifecycle E2E |
| 6 Release | TASK-023 | enabled scope complete | P0 traceability manifest and benchmark/security evidence |

Security is not deferred to one phase: authenticated identity, validation, boundedness, error/redaction and audit foundations enter at the first task that exposes the relevant boundary.

## 7. Change and completion rules

- A structural dependency/authority/data decision change is recorded in `DECISIONS.md` and, when long-lived or difficult to reverse, an ADR before code.
- Applied migration bytes are immutable. A later task adds a new migration; it never edits an applied one.
- A task may remain `DONE` only while its evidence is reproducible on the supported platform/version. A security regression reopens the task/gate.
- `PROJECT_INTAKE_REPORT.md` is refreshed for the TASK-001-complete bootstrap state and foundation toolchain/platform evidence; it MUST be refreshed again when repository or host facts change and MUST distinguish empty boundaries from implemented product behavior.
- `IMPLEMENTATION_REVIEW.md` must be updated after Phase 0 and before release with a second full review and final readiness verdict.
