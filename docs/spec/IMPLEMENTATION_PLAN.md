---
title: "梦夏（MengXia）实施计划"
project: "梦夏 / MengXia"
document_role: "Living Implementation Plan"
status: "TASK_001_DONE"
version: "0.3.5"
date: "2026-08-21"
language: "zh-CN"
source_of_truth: "IMPLEMENTATION_SPEC.md v1.1.5"
review: "IMPLEMENTATION_REVIEW.md v1.1.5"
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
| Source/workspace | TASK-001 Cargo workspace and 17 empty canonical package/binary skeletons present | domain/runtime behavior implemented by owning tasks | `FACT / PARTIAL TARGET` |
| Schema/migrations | absent | reviewed forward-only migrations | `EXPECTED_GAP` |
| Tests/CI | TASK-001 repository verification tests/scripts and arm64 macOS CI present | layered functional/security/recovery suites added by owning tasks | `FACT / PARTIAL TARGET` |
| Review | historical findings retained; no open blocker applies to TASK-001..TASK-005 | same, with reproducible evidence | `FACT` |
| Phase 0 decisions | OQ-003, early OQ-006 and foundation Client/Admin boundary accepted | retained until superseded | `DECISION / ACCEPTED` |

Current plan state: `TASK_001_DONE`. This record does not authorize TASK-002 or any later task. The whole-V1 review verdict remains `NOT READY FOR CODEX` while blocked features/open decisions exist. Implementation must follow task dependency order; later gated capabilities remain disabled.

## 3. Phase 0 — intake, evidence and decision gate

Status: `DONE` for the foundation scope required by TASK-001..TASK-005.

Required outputs:

1. `PROJECT_INTAKE_REPORT.md` with repository/toolchain/host filesystem facts and discrepancy classifications: `DONE` and refreshed after the foundation toolchain/platform decisions; refresh again when those facts change.
2. Accepted `OQ-003` decision: `DONE` in ADR-0003—Rust/MSRV 1.98.0 and bundled SQLite 3.53.4 with source/options/checksum.
3. Accepted incremental `OQ-006` caps: `DONE` for TASK-002..TASK-005 in ADR-0005; later caps close before their consuming tasks.
4. Accepted target platform and ordinary Client peer-auth contract: `DONE` in ADR-0004; Admin features explicitly remain disabled until a later OQ-010 ADR.
5. A doc traceability check covering the normative namespaces and canonical-definition/range rules in Specification §0.5. Current documentation correction defines TASK-001 IDs; `TEST-DOC-001` makes the check reproducible during TASK-001 and requires PASS before that task is `DONE`.
6. `IMPLEMENTATION_REVIEW.md` updated so no BLOCKER applies to TASK-001..TASK-005: `DONE` for current evidence.

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
| `TASK-002` Core values/error baseline | `PENDING` | FUNC-001; REQ-001, API-010, DATA-012 | TASK-001; accepted serialization limits | `mengxia-types`, domain/errors | UUID/digest/time/error property + malformed/oversize tests | No Provider/proto/DB types in domain; no raw payload/secret type |
| `TASK-003` IPC, framing, Client identity | `PENDING` | FUNC-001; API-001, API-002, API-003, API-008, API-009, API-010; SEC-005, SEC-013, SEC-014; CFG-003 | TASK-002; ADR-0004 peer-auth; ADR-0005 frame cap; Admin disabled | proto/core, framing, daemon, CLI | AC-028, AC-029; fuzz, actor spoof, unauthorized peer, cap±1, no-TCP | Actor server-derived; Admin disabled without evidence; never trust loopback/request role |
| `TASK-004` SQLite/migration engine | `PENDING` | FUNC-001; DATA-001, DATA-005, DATA-006, DATA-007, DATA-011; REL-001 | TASK-002; ADR-0003; ADR-0005 DB queue; ADR-0004 bootstrap/filesystem authority | store, `0000_store_bootstrap` | first-create target denial matrix, runtime version/options, PRAGMAs, checksum, busy/crash/corruption | Bootstrap/automatic forward migration are internal lifecycle only; no Admin RPC; do not create/modify asset migration |
| `TASK-005` BlobStorage/CAS primitives | `PENDING` | FUNC-002; DATA-002, DATA-003, DATA-004, DATA-013; PERF-001; SEC-017, SEC-021 | TASK-002; ADR-0005 stream/I/O/hash/staging caps | ports, local storage | source/symlink/disk-full/crash/O(buffer), cap±1 | Stable handles/root confinement; no source deletion; no canonical Asset ownership |
| `TASK-006` Asset domain/persistence | `PENDING` | FUNC-002, FUNC-003; REQ-001, REQ-002, REQ-004, REQ-005, REQ-008, REQ-011, REQ-012; DATA-009, DATA-010, DATA-011 | TASK-004, TASK-005 | domain/app/store, complete immutable `0001_library_assets` | AC-002, AC-005..AC-008, FK/sequence/concurrency/event tests | No migration rewrite after apply; Blob dedup never merges Asset |
| `TASK-007` copy-only ingest slice | `PENDING` | FUNC-002; REQ-010, REQ-013; API-003, API-010; REL-005 | TASK-003, TASK-006 | app/proto/CLI/store/storage | AC-001..AC-009, E2E, concurrent duplicate, all crash points | Copy only; reject adopt/reference; physical durability before registration |
| `TASK-008` verify/recovery | `PENDING` | FUNC-001, FUNC-010; REL-004, REL-008; OPS-004 | TASK-007 | daemon/app/store/storage/CLI | corruption matrix, provider-offline restart AC-015, startup cost | Deep verify explicit; unrelated local work allowed in degraded mode |
| `TASK-009` Project/Work/Take | `PENDING` | FUNC-004; REQ-003, REQ-004, REQ-006, REQ-007, REQ-012, REQ-014; SEC-014 | TASK-006 | domain/app/store/proto, `0002_projects_work` | AC-010, AC-011, AC-016; transition/concurrency/cross-Project tests | Project not tenant/Asset owner; no generic CRUD/direct state assignment |
| `TASK-010` Plugin package/Manifest | `BLOCKED` | FUNC-006; SEC-003, SEC-009, SEC-010, SEC-016, SEC-020 | TASK-001, TASK-002; OQ-010 for install/approve | package/security/schema, `0003_plugin_packages` | AC-027; schema/tamper/publisher spoof/dependency tests | VERIFIED does not authenticate publisher; exact digest grant only |
| `TASK-011` Plugin protocol/hostile fixture | `BLOCKED` | FUNC-006, FUNC-007; API-004; REL-001, REL-006; SEC-017, SEC-021 | TASK-003, TASK-010; frame/log/process caps | plugin proto/framing/host/testkit | malformed/flood/crash/timeout/queue cap suite | Private channel only; bounded stdout/stderr/frames; no Core/Admin handle |
| `TASK-012` exact OS sandbox | `BLOCKED` | FUNC-007; SEC-001, SEC-002, SEC-005, SEC-021 | TASK-011; OQ-001, OQ-002; resource caps | platform sandbox/host/security tests | AC-020..AC-023 + mandatory real hostile suite | All required dimensions ENFORCED or deny; no backend-name/self-report shortcut |
| `TASK-013` Lease/Asset Broker/audit | `BLOCKED` | FUNC-006, FUNC-007, FUNC-010; SEC-004, SEC-005, SEC-006, SEC-019; DATA-011 | TASK-009, TASK-012; OQ-010 | security/host/brokers/store, `0004_plugin_security` | AC-024, AC-026, AC-029; caller/race/revoke/audit tests | Caller/channel/run/digest binding; CAS path hidden; ordinary Client cannot grant |
| `TASK-014` controlled FFmpeg Plugin | `PENDING` | FUNC-005, FUNC-007; SEC-009, SEC-017, SEC-020; REL-006 | TASK-013; accepted executable digest/resource caps | plugin/tool + contracts | timeout/cancel/malformed media/digest/output verify | argv only, no shell/PATH/DB/CAS; output untrusted until verified |
| `TASK-015` Recipe/Run runtime | `PENDING` | FUNC-005; REQ-006, REQ-009; API-005, API-006, API-007; DATA-010; REL-002, REL-003, REL-004, REL-005, REL-006, REL-007, REL-008 | TASK-009, TASK-014; job/queue/deadline caps | runtime/domain/app/store, `0005_runtime` | AC-012..AC-016, AC-031; DAG/attempt/crash/partial-success tests | Persist intent before effect; UNKNOWN no blind retry; retry creates Attempt |
| `TASK-016` Secret/Network Brokers | `BLOCKED` | FUNC-007, FUNC-008; SEC-006, SEC-007, SEC-011, SEC-012, SEC-015, SEC-017, SEC-021; CFG-002 | TASK-013, TASK-015; OQ-004, OQ-010; egress/cost/size caps | brokers/security/config | AC-023, AC-025, AC-029, AC-044; SSRF/rebinding/redirect/canary rotation tests | No generic proxy/raw static secret to sandbox; no real egress before pass |
| `TASK-017` Provider selection gate | `BLOCKED` | FUNC-008, FUNC-012; API-005, API-006, API-007 | TASK-016; OQ-005 | ADRs, adapter README/contracts | current official source/auth/state/idempotency/webhook evidence | No implementation from memory; no generic webhook listener |
| `TASK-018` CLI Provider | `PENDING` | FUNC-008, FUNC-012; API-005, API-006, API-007; REL-002, REL-003, REL-006, REL-007 | TASK-017 | selected plugin/adapter | fake CLI + opt-in real contract, kill/timeout/env tests | verified executable/env; durable external ID; no shell |
| `TASK-019` HTTP Provider | `PENDING` | FUNC-008, FUNC-012; SEC-011, SEC-015, SEC-017; REL-002, REL-003, REL-006, REL-007 | TASK-017 | selected adapter/brokers | mock server + optional real; retry/rate/redirect/webhook-if-enabled | Broker-only egress; bounded streaming; adapter-specific callback contract |
| `TASK-020` Local/Hybrid/interoperability | `PENDING` | FUNC-012; G-001, G-003, G-008; DATA-008 | TASK-017, TASK-019 | adapter/integration/export | AC-030; reopen without plugin; relationship/resolve/register contracts | No Core schema change or provider type leakage |
| `TASK-021` Rights/classification/clearance | `BLOCKED` | FUNC-009; G-009, REQ-015, DATA-012; SEC-014, SEC-017, SEC-019 | TASK-009, TASK-013; OQ-009 | rights/domain/app/proto/store, `0006_rights_classification` | AC-040; correction/conflict/egress/cross-Project/audit E2E | UNKNOWN/CONFLICTED never implicit ALLOW; Provenance != Rights |
| `TASK-022` retention/GC/Purge | `BLOCKED` | FUNC-011; REQ-015; DATA-010, DATA-011; SEC-018, SEC-019 | TASK-008, TASK-013, TASK-021; OQ-008 | admin/app/storage/store/CLI | AC-041..AC-043; preview/hold/last-copy/concurrency/crash tests | Purge disabled until policy; exact target set; no retirement→delete inference |
| `TASK-023` release gate | `BLOCKED` | all enabled FUNC and P0 requirements | enabled tasks complete; OQ-006 release SLO part | CI/docs/evidence/ops | all mandatory suites, upgrades, benchmarks, fresh advisories | No silent skip/fabricated SLO/unsupported security claim |

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

## 6. Phases and gates

| Phase | Tasks | Entry gate | Exit evidence |
|---|---|---|---|
| 0 Intake/decisions | documentation only | repository inspected | `DONE`; ADR-0003..ADR-0005; no blocker applies to TASK-001..TASK-005 |
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
