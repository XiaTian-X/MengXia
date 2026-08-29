# MengXia 项目指南

本文件是项目入口地图，不替代详细规范。

## 必读文档

开始分析、规划或实现前，按顺序阅读：

1. `docs/spec/IMPLEMENTATION_SPEC.md` — 目标架构与规范性要求（主要 Source of Truth）
2. `docs/spec/DECISIONS.md` — 已接受决策、开放问题与冲突记录
3. `docs/spec/IMPLEMENTATION_REVIEW.md` — 当前实现可行性、安全审查与阻塞项
4. `docs/spec/IMPLEMENTATION_PLAN.md` — 当前阶段、任务顺序与验收条件
5. `docs/spec/PROJECT_INTAKE_REPORT.md` — 当前 repository/host/tooling 的只读基线证据

若文档与仓库现实不一致，不得静默解决。先记录差异，并按规范中的
`EXPECTED_GAP`、`SPEC_STALE`、`REPO_STALE`、`CONFLICT` 或 `UNKNOWN` 分类。

## 当前状态

- 项目阶段：Implementation / Phase 2 managed custody；TASK-001、TASK-002、TASK-004、TASK-003、TASK-005 complete；TASK-006 `IN_PROGRESS`
- 实现范围：V1 / MVP
- 当前仓库：TASK-001/TASK-002/TASK-004/TASK-003 已完成，TASK-003 complete evidence 保持有效；TASK-005 也已完成，其 bounded local CAS custody、opaque authority、stream/hash/write、capacity/admission、durable no-clobber publish、orphan/recovery 与 joined lifecycle 已通过本地门禁及 reviewed `macos-26` formal CI run `33073580258`
- 当前授权范围：`TASK_006_ONLY`；只允许 accepted TASK-006 proposal v0.2.2 的 Asset domain、CommandRecord/event persistence 与 immutable 0001 scope；TASK-007 及后续仍未授权

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
TASK006_LIFECYCLE: IN_PROGRESS
TASK006_IMPLEMENTATION_AUTHORITY: TASK_006_ONLY
TASK006_PROPOSAL: docs/proposals/TASK-006-GATE-PROPOSAL.md

## 工作规则

- 首次接管时先检查项目和文档，不直接修改实现代码。
- `IMPLEMENTATION_REVIEW.md` 仍有适用于当前 task 的 `BLOCKER` 或 plan 标记 `BLOCKED` 时，只能执行证据、决策和文档工作，不得初始化实现代码。
- 以 `IMPLEMENTATION_SPEC.md` 中的 `CONFIRMED` 条目为强约束。
- 架构冲突或强约束变更必须先记录到 `DECISIONS.md`，必要时建立 ADR。
- 每个实施阶段开始前确认前置决策，结束后运行相应验证命令并更新计划状态。
- 不编造标记为 `OPEN` 或 `TBD` 的选择和指标。
