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

- 项目阶段：Implementation / Phase 1 foundation；TASK-001、TASK-002、TASK-004 complete；TASK-003 pending own gate
- 实现范围：V1 / MVP
- 当前仓库：TASK-001/TASK-002/TASK-004 已完成；TASK-004 的固定 SQLite 3.53.4、配置验证、bootstrap migration、连接 hardening、精确 SQLite/platform 错误映射、完整 bootstrap schema/index/typed-row reopen validator，以及 checked-in macOS ACL shim、descriptor-relative path authority、absent/empty root creation、read-only parent preflight 与 root-mutation 前 clock/UUID first-create orchestration、durable exclusive lock/explicit release、versioned 256-byte intent codec/durable create/post-lock read/typed state、valid-intent re-fsync/exclusive empty-staging/fault-order seam、固定 SQLite child consumer、staging SQLite transaction/checkpoint/close/fsync/read-only reopen validation、descriptor-relative hard-link publish/ordered cleanup/canonical reopen、identity-bearing closed restart recovery、valid-intent 授权的空/回滚 staging 清理、可恢复 WAL/SHM matrix、bounded pre-open required-commit WAL 损坏分类、16×256 多连接 WAL-reset stress、23-point/29-case same-OS SIGKILL recovery matrix、完整 deterministic corruption matrix，以及 bounded writer/read admission 与 joined shutdown lifecycle 已实现；本机 developer gate 与 reviewed runner-XIP formal CI run `32695815747` 全部通过
- 当前授权范围：Specification v1.1.14 与 Plan completion record 将 TASK-004 标记为 `DONE`。TASK-003 及后续 task 仍未授权；开始 TASK-003 前必须单独读取其完整定义、建立稳定 AC/TEST registry 和 start record，不得从 TASK-004 completion 自动推导授权

## 工作规则

- 首次接管时先检查项目和文档，不直接修改实现代码。
- `IMPLEMENTATION_REVIEW.md` 仍有适用于当前 task 的 `BLOCKER` 或 plan 标记 `BLOCKED` 时，只能执行证据、决策和文档工作，不得初始化实现代码。
- 以 `IMPLEMENTATION_SPEC.md` 中的 `CONFIRMED` 条目为强约束。
- 架构冲突或强约束变更必须先记录到 `DECISIONS.md`，必要时建立 ADR。
- 每个实施阶段开始前确认前置决策，结束后运行相应验证命令并更新计划状态。
- 不编造标记为 `OPEN` 或 `TBD` 的选择和指标。
