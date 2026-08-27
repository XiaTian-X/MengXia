---
title: "梦夏（MengXia）决策日志"
project: "梦夏 / MengXia"
document_role: "Decision Log and ADR Index"
status: "ACTIVE"
version: "0.3.18"
date: "2026-08-26"
language: "zh-CN"
---

# 梦夏（MengXia）决策日志

本文件记录已接受决策、开放问题、规范冲突和 ADR 索引。详细规范仍以
`IMPLEMENTATION_SPEC.md` 为主要 Source of Truth。

## 已接受的基线决策

下列基线始于 canonical specification v1.0.1，并包含至 v1.1.18 的独立审查、foundation gate、TASK-001/TASK-002/TASK-004/TASK-003 completion、TASK-004-before-TASK-003 authority sequencing，以及 accepted TASK-005 pre-start contract；完整约束与理由见当前规范、accepted supplement 和 Review 记录。

| ID | 决策 | 状态 | 来源 |
|---|---|---|---|
| `BASE-001` | V1 使用 Rust、Tokio、SQLite、proto3、JSON Schema 2020-12 与 Cargo Workspace | `ACCEPTED` | Implementation Spec §0.4 |
| `BASE-002` | 产品采用 local-first、vendor-neutral 架构 | `ACCEPTED` | Executive Summary |
| `BASE-003` | V1 不实现 UI，所有动作通过 CLI + Core API 完成 | `ACCEPTED` | §3 |
| `BASE-004` | Canonical persistence 使用 State + append-only events + rebuildable projections + Blob Storage | `ACCEPTED` | `DATA-001` |
| `BASE-005` | 第三方或未知 Native Plugin 必须受 OS 强制 sandbox，否则拒绝激活 | `ACCEPTED` | `SEC-002` |
| `BASE-006` | 真实 Provider Credential 和网络接入晚于安全隔离、Lease 与 Broker | `ACCEPTED` | Executive Summary |
| `BASE-007` | V1 为 single-Library、single-local-owner trust domain；Project 不是 tenant | `ACCEPTED V1` | `DEC-016`, review `REVIEW-002` |
| `BASE-008` | actor 只能由已认证 IPC channel 派生；request 不得声明 actor/Admin role | `ACCEPTED` | `DEC-017`, review `REVIEW-001` |
| `BASE-009` | 初始 IngestAsset vertical slice 仅支持非破坏性的 copy mode | `ACCEPTED V1` | `DEC-018`, review `REVIEW-009` |
| `BASE-010` | 有限 safety cap 在使用它的 task 前阻塞；性能 SLO 仍须测量 | `ACCEPTED` | `DEC-019`, review `REVIEW-008` |
| `BASE-011` | Foundation pin Rust/MSRV 1.98.0 and bundled SQLite 3.53.4 with verified official source/checksum | `ACCEPTED` | `ADR-0003`, `OQ-003` |
| `BASE-012` | arm64 macOS is the initial foundation platform; ordinary Client is UID/channel-derived; Admin and third-party Native Plugin remain disabled | `ACCEPTED V1 FOUNDATION` | `ADR-0004` |
| `BASE-013` | TASK-002..TASK-005 use finite configurable foundation caps; later Plugin/Provider/release caps remain open | `ACCEPTED PARTIAL` | `ADR-0005`, `OQ-006` |
| `BASE-014` | Stable TEST obligations are declared before task start, implemented during the owning task and executable with PASS evidence before DONE | `ACCEPTED` | Specification §0.5, `REVIEW-016` |
| `BASE-015` | First-create bootstrap accepts an absent or correctly owned empty target and rejects canonical/non-empty/unsafe targets | `ACCEPTED` | `ADR-0004`, `REVIEW-018` |
| `BASE-016` | Whole-V1 readiness and scoped task authorization are separate; completed TASK-001/TASK-002 evidence does not authorize a later task whose own gate is absent | `ACCEPTED` | `REVIEW-019`, Plan v0.3.7 |
| `BASE-017` | TASK-004 creates durable Library owner/lock context before TASK-003 activates local Client IPC; IPC consumes the context without depending on SQLite | `ACCEPTED` | user-selected Option A; Specification v1.1.8; TASK-003 gate analysis |
| `BASE-018` | TASK-005 local custody uses opaque source/root capabilities, atomic logical/physical reservation, exact-case no-clobber CAS, stable backend-instance identity and fail-closed cleanup; its accepted contract does not activate implementation | `ACCEPTED` | ADR-0007; Specification v1.1.18; TASK-005 supplement |

## 开放决策

不得在实现中静默替这些项目作决定。开始依赖相关选择的任务前，应补充候选方案、证据、影响和结论。

Canonical Open Question ID 以规范 §24 的 `OQ-*` 为准；本表不得建立第二套编号。

| ID | 问题 | 阻塞范围 | 状态 |
|---|---|---|---|
| `OQ-001` / `OQ-002` | arm64 macOS foundation 已接受；exact sandbox backend/version 与第三方 Native Plugin release claim | TASK-012、第三方 Native Plugin claim | `PARTIAL / LATER BLOCKING` |
| `OQ-003` | Rust/MSRV 与包含 WAL-reset 修复的 bundled SQLite 版本/编译选项/checksum | TASK-001、TASK-004 | `ACCEPTED / ADR-0003` |
| `OQ-004` | canonical Credential store | TASK-016、真实 Provider | `OPEN / BLOCKING` |
| `OQ-005` | 真实 Provider validation targets | TASK-017..TASK-020 | `OPEN / BLOCKING` |
| `OQ-006` | TASK-002..TASK-005 foundation caps 已接受；Plugin/Provider caps、reference hardware 与 release SLO | TASK-011/TASK-012/TASK-016；release | `PARTIAL / LATER BLOCKING` |
| `OQ-007` | user-installed third-party code 是否可为 TRUSTED_NATIVE | policy/release claim | `OPEN / NON-BLOCKING WITH SAFE DEFAULT DENY/SANDBOX_ONLY` |
| `OQ-008` | retention、hold、orphan 与 raw observation policy | TASK-022、production | `OPEN / BLOCKING` |
| `OQ-009` | rights/data-classification schema | TASK-021、真实 egress | `OPEN / BLOCKING` |
| `OQ-010` | Foundation 明确禁用 Admin；未来 macOS Admin authority/user-presence mechanism | TASK-010/TASK-013/TASK-016/TASK-022 | `DEFERRED / ADMIN DISABLED / LATER BLOCKING` |

## 冲突记录

发现规范、仓库、官方协议或已接受决策之间的冲突时，追加以下模板：

```text
CONFLICT:
Source A:
Source B:
Recommended canonical decision:
Reason:
Impact:
Classification: EXPECTED_GAP | SPEC_STALE | REPO_STALE | CONFLICT | UNKNOWN
Status: OPEN | RESOLVED
```

### TASK-003/TASK-004 durable owner authority sequencing

```text
CONFLICT:
Source A: TASK-003 required peer UID to equal the recorded Library owner UID and was listed before TASK-004.
Source B: ADR-0004 and TASK-004 assign first durable Library owner creation, Library lock and bootstrap lifecycle to TASK-004.
Recommended canonical decision: execute TASK-004 before TASK-003; TASK-003 receives an already-open Library context and never persists, infers or reads owner authority from request/configuration.
Reason: avoids both a store-boundary violation and a temporary authentication root that would weaken ADR-0004.
Impact: only the dependency order changes. Stable Task IDs/scopes remain; TASK-006/TASK-007/TASK-011 and later dependency semantics remain intact and the graph stays acyclic.
Classification: CONFLICT
Resolution evidence: user accepted Option A on 2026-08-21; repository document dependency tests enforce the accepted edges and reject cycles.
Status: RESOLVED / BASE-017
```

### TASK-005 default Blob root versus TASK-004 exact namespace

```text
CONFLICT:
Source A: canonical configuration fixes `MENGXIA_BLOB_ROOT` default to `<library_root>/storage`.
Source B: completed TASK-004 opens and synchronizes only the exact `.mengxia.lock + library.sqlite3` completed state.
Recommended canonical decision: accept ADR-0007 and the TASK-005 supplement; permit one exact descriptor-verified `storage` directory only beside the completed canonical state, retain every bootstrap/recovery denial, and bound enumeration before collecting an unexpected fourth candidate.
Reason: changing the default causes downstream configuration drift, while a permissive namespace would weaken completed filesystem authority.
Impact: TASK-005 depends on TASK-004 and owns the narrow platform/store compatibility implementation and regression tests. No current code is retroactively noncompliant because no TASK-005 storage state exists; TASK-004 remains DONE until the TASK-005 activation implements the accepted extension.
Classification: CONFLICT plus canonical TASK-005 dependency/scope `SPEC_STALE`; repository CAS absence remains `EXPECTED_GAP`.
Status: RESOLVED / BASE-018 / ADR-0007 / SPECIFICATION v1.1.18
```

### `BASELINE-001` Git repository 初始化

```text
CONFLICT:
Source A: Implementation Specification v1.0.1 记录当前工作区没有 Git 元数据。
Source B: 2026-08-20 已按用户要求初始化本地 Git repository，默认分支为 main。
Recommended canonical decision: 更新仓库现状描述；不改变任何目标架构或 CONFIRMED 决策。
Reason: 这是完成 repository bootstrap 后产生的正常状态变化。
Impact: 规范中的仓库现状改为“Git 已初始化，源代码结构尚未初始化”。
Classification: SPEC_STALE
Status: RESOLVED
```

### `BASELINE-002` 文档基线 commit history

```text
CONFLICT:
Source A: PROJECT_INTAKE_REPORT v1.2.1 still stated that the initialized repository had no commits.
Source B: Git history contains the reviewed documentation baseline commit created on 2026-08-21.
Recommended canonical decision: refresh only the Current State evidence; keep TASK-001 as the first implementation/CI bootstrap task.
Reason: a documentation commit does not mean source, tests or CI exist, but “no commits” is no longer factual.
Impact: PROJECT_INTAKE_REPORT v1.2.2 records commit history without changing target architecture or implementation readiness.
Classification: SPEC_STALE
Status: RESOLVED
```

### `BASELINE-003` TASK-001 后仓库现状

```text
CONFLICT:
Source A: AGENTS.md、Implementation Specification/Review、Implementation Plan Current State 与 PROJECT_INTAKE_REPORT 仍把仓库描述为仅有文档、没有 Cargo workspace、测试或 CI，且把 TASK-001 记为下一步。
Source B: 当前工作区已存在 TASK-001 的 Cargo workspace、17 个 canonical package/binary skeleton、repository verification tests/scripts、CI、Cargo.lock 与 policy 配置；Implementation Plan 已将 TASK-001 标记为 DONE。
Recommended canonical decision: 将所有 Current State 证据同步为“TASK-001 bootstrap 已实现并验证；尚无 TASK-002 domain behavior、schema、migration 或产品能力”，保留后续 task gate，不把空骨架误报为功能实现。
Reason: TASK-001 完成是正常仓库状态变化；陈旧入口会使后续 Codex 重复 bootstrap 或错误判断 task authorization。
Impact: 同步更新 AGENTS.md、Specification v1.1.5、Review v1.1.5、Plan v0.3.5 与 Intake Report v1.3.0；TASK-002 仍为 PENDING，开始前必须建立其稳定 AC/TEST registry 与 start record。
Classification: SPEC_STALE
Status: RESOLVED
```

### `BASELINE-004` TASK-004 本地门禁后 canonical current-state 与 future-task mapping

```text
CONFLICT:
Source A: Specification v1.1.11 的 Current State/Repository Map 仍写 17 个 package、无 schema/migration/Library owner-lock，并在 §18 将 TASK-007 收窄到 `AC-001..AC-006`；将跨后续 Broker/Secret task 的 `AC-024..AC-026` 放入 TASK-012 测试范围。
Source B: commit abe48db 后仓库已有第 18 个 mengxia-platform-fs package、完整 TASK-004 本地实现与 14-ID 本地 PASS；Plan 的 TASK-007 为 `AC-001..AC-009`，TASK-010 为 `AC-027`，TASK-012 为 `AC-020..AC-023`；formal reviewed CI attestation 仍未产生。
Recommended canonical decision: 只同步可观察的 Current State、repository map 与既有 AC 语义映射；TASK-004 保持 IN_PROGRESS，AC-065/TEST-SUPPLY-004/final DONE 继续等待正式 reviewed CI evidence；不改变任何生产实现、迁移字节、依赖顺序、权限边界或后续 task authorization。
Reason: 陈旧 canonical facts 会误导后续实现，而本地证据不能替代 accepted formal CI boundary。
Impact: Specification v1.1.12、Review v1.1.22、Plan v0.3.22、Intake v1.3.17、accepted supplement/ADR references 与 document traceability 同步；实现行为不变。
Classification: SPEC_STALE
Status: RESOLVED
```

### `BASELINE-005` TASK-004 formal CI Xcode distribution provenance

```text
CONFLICT:
Source A: accepted manifest/preflight pinned clang and libtool bytes observed from the local Mac App Store Xcode 26.6 installation and treated them as the formal CI tuple.
Source B: reviewed `macos-26-arm64` image `20260728.0273.1` installs the same Xcode 26.6 build 17F113 from runner-images `Xcode_26.6_Universal` XIP, but its clang/libtool bytes have different stable SHA-256 values; the original CI stopped fail-closed before Cargo.
Recommended canonical decision: retain separate evidence classes; keep default local builds non-attested, and pin only the formal attested profile to the reviewed runner XIP tool bytes. Emit observed non-secret tuple/digests before comparison without accepting fallback values.
Reason: distribution-equivalent version banners do not prove byte identity. Formal evidence must be established on its actual CI distribution, while local development must remain usable without pretending to provide release attestation.
Impact: Specification v1.1.13, ADR-0006 clarification, accepted supplement, provenance manifest, preflight/build constants and supply/document tests synchronize. Runtime behavior, migration bytes, public API and downstream authorization are unchanged.
Classification: CONFLICT
Status: RESOLVED
```

### `BASELINE-006` TASK-004 formal completion evidence

```text
CONFLICT:
Source A: canonical current-state records kept TASK-004 IN_PROGRESS while awaiting corrected reviewed CI attestation.
Source B: commit bfcb151 and reviewed GitHub Actions run 32695815747 passed the exact runner-XIP preflight, retained TASK-001 baseline and all fourteen TASK-004 gates.
Recommended canonical decision: mark TASK-004 DONE with per-TEST, per-AC and applicable security PASS evidence; keep TASK-003 PENDING until its separate stable registry/start gate is reviewed.
Reason: the final external evidence requirement is satisfied, but task completion never grants implicit authority to a dependent task.
Impact: Specification v1.1.14、Review v1.1.24、Plan v0.3.24、Intake v1.3.19、AGENTS current state and document traceability synchronize. No runtime behavior, migration, public API or later implementation changes.
Classification: SPEC_STALE
Status: RESOLVED
```

### `REVIEW-GAP-003` TASK-002 public contract and stable start registry

```text
CONFLICT:
Source A: Specification v1.1.5 and Plan v0.3.5 identified TASK-002's broad value/error goal but did not define stable AC/TEST IDs, exact public signatures, fallible ID generation, canonical parser boundaries or the minimal dependency feature set required before implementation.
Source B: The user-reviewed TASK-002 start-gate proposal fixes those contracts, adds the five error taxonomy rows already required by canonical operation prose or the accepted value boundaries, and supplies a complete start record and verification matrix.
Recommended canonical decision: incorporate the accepted proposal into Specification v1.1.6 and Plan v0.3.6, authorize TASK-002 only, and retain every TASK-003+ gate.
Reason: this closes the mandatory pre-start traceability/public-contract gap without changing an accepted authority, persistence, Provider or destructive-operation architecture decision.
Impact: AC-055 through AC-059 and seven TASK-002 TEST obligations become canonical; exact uuid/getrandom/time/proptest pins and features are accepted for this task; synchronized Current State documents mark TASK-002 IN_PROGRESS only after TEST-DOC-001 and the TASK-001 baseline pass.
Classification: EXPECTED_GAP
Resolution evidence: user accepted the modified `docs/proposals/TASK-002-GATE-PROPOSAL.md` on 2026-08-21; official locked crate source/metadata confirmed the specified Builder/fallible entropy APIs, licenses and MSRVs.
Status: RESOLVED
```

### `REVIEW-CONFLICT-001` Caller-supplied actor 与 channel-bound identity

```text
CONFLICT:
Source A: Implementation Specification v1.0.1 CommandEnvelope 允许 caller 填写 actor_principal。
Source B: 同一规范 SEC-005/§12.3 要求 authority domain 与 channel/process-bound identity，且不信任 self-report。
Recommended canonical decision: reserve wire field 3；Core 仅从 authenticated channel 派生 PrincipalContext。
Reason: caller field 无法作为认证或审计证据。
Impact: TASK-003 增加 actor spoof/peer/Admin negative tests；Admin 未解决 OQ-010 时禁用。
Classification: CONFLICT
Status: RESOLVED / BASE-008
```

### `REVIEW-CONFLICT-002` 初始 ingest mode 与 Managed custody

```text
CONFLICT:
Source A: TASK-007 列出 copy/adopt/reference，但未定义 destructive 与 custody 语义。
Source B: DATA-002/DATA-003 要求 Managed Asset 有 verified durable user-controlled Location 且 physical durability 先于注册。
Recommended canonical decision: initial vertical slice 仅 copy；reference 明确为 UNMANAGED，adopt 需独立 destructive contract。
Reason: 防止源文件移动/删除或非 durable reference 被误报为 Managed。
Impact: TASK-007 与 AC-009 修订；未来新增 mode 需 ADR/contract。
Classification: CONFLICT
Status: RESOLVED / BASE-009
```

### `REVIEW-CONFLICT-003` Migration ownership

```text
CONFLICT:
Source A: TASK-004 创建 0001_library_assets.sql；TASK-006 后续仍需修改它。
Source B: DATA-007 与 §22.1 禁止修改已合并 migration bytes。
Recommended canonical decision: TASK-004 只拥有 0000_store_bootstrap；TASK-006 一次性拥有完整 0001_library_assets。
Reason: 保证 checksum 与 upgrade fixture 可重现。
Impact: migration 序号与 task ownership 已修订。
Classification: CONFLICT
Status: RESOLVED
```

### `REVIEW-GAP-001` Phase 0 blocking decisions

```text
CONFLICT:
Source A: IMPLEMENTATION_PLAN v0.1.0 可被理解为 Phase 0 后直接初始化 Cargo workspace。
Source B: OQ-003/OQ-006 阻塞 TASK-001/TASK-003/TASK-004，OQ-010 阻塞 Admin authority；SQLite 官方记录当前 WAL-reset fixed-version requirement。
Recommended canonical decision: Phase 0 blocks implementation until the current task's dependent OQ/Review gates close. ADR-0003..ADR-0005 now allow TASK-001 to begin while later Admin, third-party Native Plugin, Credential, Provider and destructive capabilities remain fail-closed at their own gates.
Reason: Codex may not guess security versions, identity mechanisms or resource limits; accepted foundation evidence is sufficient only for TASK-001..TASK-005.
Impact: Phase 0 foundation scope is complete without prematurely closing OQ-002/OQ-004/OQ-005/OQ-008/OQ-009/OQ-010 or later OQ-006 portions.
Classification: EXPECTED_GAP
Resolution evidence: user accepted the recommended foundation boundary on 2026-08-21; ADR-0003..ADR-0005 record exact evidence and behavior.
Status: RESOLVED
```

### `REVIEW-CONFLICT-004` First-create Library bootstrap vs disabled Admin

```text
CONFLICT:
Source A: ADR-0004 disables every Admin-sensitive operation until OQ-010 closes.
Source B: TASK-004 and InitializeLibrary require a path to create LibraryMeta/owner state and apply deterministic startup migrations before an authenticated Client channel exists.
Recommended canonical decision: first-create bootstrap is a one-shot internal daemon lifecycle path authorized by the daemon effective UID plus strict local parent/target checks; deterministic checksummed forward migrations are internal startup lifecycle. Neither is an ordinary Client/Admin RPC. Manual/destructive migration administration remains disabled.
Reason: avoids a bootstrap-authentication cycle without weakening the Admin boundary or exposing DB access to CLI.
Impact: Specification v1.1.2, ADR-0004 and TASK-004 define target denial tests and fail-closed behavior.
Classification: CONFLICT
Status: RESOLVED
```

### `REVIEW-GAP-002` TASK-001 stable acceptance/test IDs

```text
CONFLICT:
Source A: Specification/Plan required a task to reference stable acceptance/test IDs before implementation.
Source B: TASK-001 used only natural-language “workspace builds”, “architecture test” and “smoke build” labels.
Recommended canonical decision: define immutable AC/TEST identifier rules; bind TASK-001 to AC-050 through AC-054 and its six TEST IDs; require declaration before start and executable PASS evidence before DONE.
Reason: deterministic completion cannot depend on mutable prose, but requiring commands before TASK-001 creates them would be circular.
Impact: TASK-001 remains next; its obligations are stable at start and its repository checks must exist and pass before completion.
Classification: EXPECTED_GAP
Resolution evidence: `REVIEW-016`; Specification v1.1.4 / Review v1.1.4 / Plan v0.3.4.
Status: RESOLVED
```

### `REVIEW-CONFLICT-005` Accepted foundation caps missing from configuration model

```text
CONFLICT:
Source A: ADR-0005 accepts finite configurable decode, DB, ingest, staging and free-space boundaries; Specification §17 forbids handler/adapter magic constants.
Source B: Specification v1.1.3 §16 omitted several accepted keys and did not state which values could only tighten.
Recommended canonical decision: expose every configurable ADR-0005 boundary as a typed key and make maximum/reserve widening impossible without a new decision.
Reason: otherwise implementers must hard-code or invent configuration/range behavior.
Impact: §16 and ADR-0005 now define matching defaults, ranges and tightening-only semantics; invalid combinations disable the dependent operation.
Classification: CONFLICT
Resolution evidence: `REVIEW-017`; Specification v1.1.4 and clarified ADR-0005.
Status: RESOLVED
```

### `REVIEW-CONFLICT-006` Empty bootstrap target acceptance

```text
CONFLICT:
Source A: ADR-0004 and Specification §12.3 allow an absent or correctly owned empty first-create target.
Source B: TASK-004 and ADR-0004 verification said bootstrap rejects an “existing” target, which also includes the allowed empty target.
Recommended canonical decision: reject existing canonical metadata, non-empty content and unsafe ownership/mode/symlink/filesystem states, while accepting an absent or correctly owned empty target.
Reason: target existence alone is not the authority/integrity risk; ambiguous wording would produce incompatible bootstrap implementations.
Impact: TASK-004 and ADR-0004 verification now use the same denial matrix.
Classification: CONFLICT
Resolution evidence: `REVIEW-018`; Specification v1.1.4 and clarified ADR-0004.
Status: RESOLVED
```

### `REVIEW-CONFLICT-007` Whole-V1 verdict vs scoped TASK-001 authorization

```text
CONFLICT:
Source A: the mandated final review contract restricts functional/security/Codex verdicts to exact whole-product enum values and requires NOT READY FOR CODEX while blockers remain.
Source B: Review v1.1.3 placed custom foundation/task-scoped READY values in those rows although later V1 features remained BLOCKED.
Recommended canonical decision: report whole-V1 as FUNCTIONAL/SECURITY CONDITIONALLY READY and CODEX NOT READY FOR CODEX; report READY FOR TASK-001 separately as the current authorized slice.
Reason: readiness scope must neither bypass later gates nor prevent safe foundation progress.
Impact: Review v1.1.4 and Plan v0.3.4 distinguish the overall verdict from task authorization; all later gates remain unchanged.
Classification: CONFLICT
Resolution evidence: `REVIEW-019`.
Status: RESOLVED
```

## TASK-003 gate acceptance — 2026-08-25

The user accepted `docs/proposals/TASK-003-GATE-PROPOSAL.md` as the normative
TASK-003 supplement. Its exact wire/framing/decode-depth contract, 5-second deadline,
32-handshake admission cap, opaque opened-Library seam, owner-only marked runtime
namespace, two-command CLI, descriptor-first offline generation policy, formal
second-UID completion evidence and five-AC/eleven-TEST registry are binding. TASK-003
alone is `IN_PROGRESS`; no later task is authorized.

The accepted error conflict preserves existing storage and Provider/Plugin meanings
and adds three local IPC codes with the exact Specification §14.1 rows:

TASK003_ERROR_TAXONOMY_CONFLICT: ACCEPTED
TASK003_ERROR_CODES_ADDED: IPC_TRANSPORT_ERROR; PROTOCOL_VERSION_UNSUPPORTED; DEADLINE_EXCEEDED
TASK003_STORAGE_IO_SOURCE_PRESERVED: filesystem/backend
TASK003_UNSUPPORTED_CAPABILITY_SOURCE_PRESERVED: declared Provider/Plugin capability contract

The accepted cross-task AC correction records contributor evidence without permitting
premature completion or implementation:

TASK003_AC_OWNERSHIP_CONFLICT: ACCEPTED
TASK003_AC_028_CONTRIBUTORS: TASK-003; TASK-007
TASK003_AC_028_TERMINAL_OWNER: TASK-013
TASK003_AC_029_CONTRIBUTORS: TASK-003; TASK-013; TASK-016; TASK-022
TASK003_AC_029_TASK013_BRANCHES: PLUGIN_GRANT; AUDIT_EXPORT; MANUAL_MIGRATION_ADMIN
TASK003_AC_029_TASK016_BRANCHES: CREDENTIAL
TASK003_AC_029_TASK022_BRANCHES: PURGE
TASK003_AC_029_TERMINAL_OWNER: TASK-023

TASK003_CANONICAL_GATE: ACCEPTED
TASK003_SPECIFICATION_VERSION: 1.1.17
TASK003_LIFECYCLE: DONE
TASK003_PROPOSAL: docs/proposals/TASK-003-GATE-PROPOSAL.md

### TASK-003 formal executable preflight path — 2026-08-26

```text
CONFLICT:
Source A: the accepted TASK-003 supplement required absolute /usr/bin/test for the formal second-UID executable preflight.
Source B: reviewed macOS 26 CI run 32912547078 and the accepted developer host both report /usr/bin/test absent and /bin/test present.
Recommended canonical decision: use absolute /bin/test -x for that private formal preflight only.
Reason: preserve the exact no-PATH, cleared-environment executable check with the platform's real utility path.
Impact: no production behavior, authority boundary, public interface, dependency, AC/TEST ownership or later-task authorization changes.
Classification: SPEC_STALE
Status: RESOLVED / SPECIFICATION v1.1.16
```

### TASK-003 completion evidence — 2026-08-26

- Accepted result: TASK-003 is `DONE` under Specification v1.1.17 without changing
  the previously accepted wire, identity, filesystem authority, configuration,
  dependency or later-task boundaries.
- Evidence: implementation commit
  `4f7bf27855b05c5080790aae3221ee10ae662431`; reviewed GitHub Actions run
  `32914222948`; formal job `task-003-second-uid`; runner `macos-26`; result `PASS`.
- Authority consequence: this closes TASK-003 only. TASK-005 and every later task
  remain unauthorized until their own canonical start gates are accepted.

## TASK-005 gate acceptance — 2026-08-26

Independent review accepts `docs/proposals/TASK-005-GATE-PROPOSAL.md` and ADR-0007
as the normative pre-start TASK-005 contract. The decision fixes opaque local source
and root authority, config-authority binding, bounded control/streaming, atomic
logical plus physical remaining-byte admission, exact-case APFS namespace,
no-clobber/full-sync promotion, stable Location backend identity, cleanup-failed
runtime closure, exact error/retry mapping and fixed crash/fault registries.

The same review corrects four planning defects before implementation: TASK-005 now
depends on completed TASK-004; the default `storage` namespace compatibility is
completed-state-only and bounded; the 85-byte internal locator gives Blob roots a
937-byte absolute limit; and formal gate aggregation invokes the retained
TASK-001/002/004 baseline once through `verify-task-003.sh` rather than recursively.

Lifecycle consequence: the contract, AC-074 through AC-081 and all seventeen TEST
IDs are canonical. The later explicit activation recorded below changes TASK-005 to
`IN_PROGRESS` without changing this accepted contract. TASK-006 and later tasks
remain unauthorized.

TASK005_CANONICAL_GATE: ACCEPTED
TASK005_SPECIFICATION_VERSION: 1.1.18
TASK005_LIFECYCLE: IN_PROGRESS
TASK005_IMPLEMENTATION_AUTHORITY: TASK_005_ONLY
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md

Activation evidence: on 2026-08-27 the user explicitly authorized implementation
after review of the synchronized contract. Proposal §16.1 is copied into the Plan;
TASK-005 alone is `IN_PROGRESS`. This changes no accepted contract and grants no
TASK-006 or later authority.

Implementation evidence update — 2026-08-27: the exact TASK-005 implementation and
all seventeen executable TEST mappings now pass both the local developer gate and a
complete local `formal` candidate run on APFS, including 30 KILL points, 78 fault
seams and generated 1/10/100 GiB O(buffer) evidence. This is not the reviewed
`macos-26` CI attestation required for `DONE`; lifecycle and authority therefore
remain unchanged.

## ADR 索引

TASK-004 gate acceptance on 2026-08-22 resolves the remaining build-host mismatch:
formal CI retains exact source/tool/path/digest evidence, while ordinary developer
builds are explicitly non-attested. Xcode components may be owned only by root or
the recorded admin build eUID and may never be group/world writable. This is the
accepted finite build-host boundary in `ADR-0006`; runtime Library path/ACL/lock
requirements are unchanged. The accepted TASK-004 supplement and synchronized Plan
start record authorize TASK-004 alone.

TASK-004 implementation evidence additionally found that SQLite 3.53.4 accepts the
mandatory `SQLITE_TRUSTED_SCHEMA=0` compiler define but does not publish that define
through `sqlite3_compileoption_get`/`PRAGMA compile_options`. Requiring a nonexistent
registry row was `SPEC_STALE`, not an implementation failure. The accepted correction
keeps the define and proves both `SQLITE_DBCONFIG_TRUSTED_SCHEMA=false` and
`PRAGMA trusted_schema=0` on every connection; the other five repository defines
remain compile-option assertions. This changes no security boundary.

重大、长期或难以逆转的决策应建立单独 ADR，并在此登记。

| ADR | 标题 | 状态 | 日期 |
|---|---|---|---|
| `ADR-0001` | V1 local authority and identity boundary | `ACCEPTED` | 2026-08-20 |
| `ADR-0002` | Copy-only initial Managed Asset ingest | `ACCEPTED` | 2026-08-20 |
| `ADR-0003` | Foundation Rust toolchain and bundled SQLite | `ACCEPTED` | 2026-08-21 |
| `ADR-0004` | arm64 macOS foundation Client authority and deferred Admin | `ACCEPTED` | 2026-08-21 |
| `ADR-0005` | Foundation finite safety caps | `ACCEPTED` | 2026-08-21 |
| `ADR-0006` | macOS filesystem FFI and build-evidence boundary | `ACCEPTED` | 2026-08-22 |
| `ADR-0007` | Local CAS custody and capability boundary | `ACCEPTED` | 2026-08-26 |

建议命名：`docs/spec/adr/ADR-0001-short-title.md`。

## ADR 最小模板

```markdown
# ADR-NNNN: 标题

- Status: PROPOSED | ACCEPTED | SUPERSEDED | REJECTED
- Date: YYYY-MM-DD
- Supersedes: 可选

## Context

## Decision

## Consequences

## Verification
```
