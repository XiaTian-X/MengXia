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

- 项目阶段：Implementation / Phase 2 managed custody；TASK-001、TASK-002、TASK-004、TASK-003、TASK-005、TASK-006、TASK-007 complete；post-TASK-007 精确纠正集及 ADR-0010 编排已通过 reviewed `macos-26` CI run `33482363576`
- 实现范围：V1 / MVP
- 当前仓库：TASK-001/TASK-002/TASK-004/TASK-003/TASK-005/TASK-006/TASK-007 已完成；TASK-007 additive protocol 1.1 copy-ingest、bounded orchestration、durable CAS/Asset registration 与恢复证据已通过本地门禁及 reviewed `macos-26` formal CI run `33401785647`
- 当前授权范围：`NONE`；ADR-0010 / `REVIEW-CONFLICT-023` 的 CI workflow、验证脚本、测试与文档维护已完成本地及 reviewed CI 验证；产品实现、root rebind、TASK-008 生产实现及后续仍未授权

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
TASK007_LIFECYCLE: DONE
TASK007_IMPLEMENTATION_AUTHORITY: NONE
TASK007_PROPOSAL: docs/proposals/TASK-007-GATE-PROPOSAL.md

## 工作规则

- 首次接管时先检查项目和文档，不直接修改实现代码。
- `IMPLEMENTATION_REVIEW.md` 仍有适用于当前 task 的 `BLOCKER` 或 plan 标记 `BLOCKED` 时，只能执行证据、决策和文档工作，不得初始化实现代码。
- 以 `IMPLEMENTATION_SPEC.md` 中的 `CONFIRMED` 条目为强约束。
- 架构冲突或强约束变更必须先记录到 `DECISIONS.md`，必要时建立 ADR。
- 每个实施阶段开始前确认前置决策，结束后运行相应验证命令并更新计划状态。
- 不编造标记为 `OPEN` 或 `TBD` 的选择和指标。
