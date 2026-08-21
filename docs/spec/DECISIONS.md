---
title: "梦夏（MengXia）决策日志"
project: "梦夏 / MengXia"
document_role: "Decision Log and ADR Index"
status: "ACTIVE"
version: "0.3.4"
date: "2026-08-21"
language: "zh-CN"
---

# 梦夏（MengXia）决策日志

本文件记录已接受决策、开放问题、规范冲突和 ADR 索引。详细规范仍以
`IMPLEMENTATION_SPEC.md` 为主要 Source of Truth。

## 已接受的基线决策

下列基线始于 canonical specification v1.0.1，并包含至 v1.1.4 的独立审查与 foundation gate 修订；完整约束与理由见当前规范和 Review 记录。

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
| `BASE-016` | Whole-V1 readiness verdict and current task authorization are separate; full V1 is not ready while gated features remain, but TASK-001 is authorized | `ACCEPTED` | `REVIEW-019`, Plan v0.3.4 |

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

## ADR 索引

重大、长期或难以逆转的决策应建立单独 ADR，并在此登记。

| ADR | 标题 | 状态 | 日期 |
|---|---|---|---|
| `ADR-0001` | V1 local authority and identity boundary | `ACCEPTED` | 2026-08-20 |
| `ADR-0002` | Copy-only initial Managed Asset ingest | `ACCEPTED` | 2026-08-20 |
| `ADR-0003` | Foundation Rust toolchain and bundled SQLite | `ACCEPTED` | 2026-08-21 |
| `ADR-0004` | arm64 macOS foundation Client authority and deferred Admin | `ACCEPTED` | 2026-08-21 |
| `ADR-0005` | Foundation finite safety caps | `ACCEPTED` | 2026-08-21 |

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
