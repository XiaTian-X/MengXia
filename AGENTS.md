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

- 项目阶段：Implementation / Phase 1 foundation；TASK-001、TASK-002 complete
- 实现范围：V1 / MVP
- 当前仓库：TASK-001/TASK-002 已完成；TASK-004 的固定 SQLite 3.53.4、配置验证、bootstrap migration、连接 hardening、精确 SQLite/platform 错误映射、完整 bootstrap schema/index/typed-row reopen validator，以及 checked-in macOS ACL shim、descriptor-relative path authority、absent/empty root creation、read-only parent preflight 与 root-mutation 前 clock/UUID first-create orchestration、durable exclusive lock/explicit release、versioned 256-byte intent codec/durable create/post-lock read/typed state、valid-intent re-fsync/exclusive empty-staging/fault-order seam、固定 SQLite child consumer、staging SQLite transaction/checkpoint/close/fsync/read-only reopen validation、descriptor-relative hard-link publish/ordered cleanup/canonical reopen、identity-bearing closed restart recovery、valid-intent 授权的空/回滚 staging 清理、可恢复 WAL/SHM matrix、bounded pre-open required-commit WAL 损坏分类、16×256 多连接 WAL-reset stress、23-point/29-case same-OS SIGKILL recovery matrix、完整 deterministic corruption matrix，以及 bounded writer/read admission 与 joined shutdown lifecycle 已实现；14 个 gate 的统一脚本和 CI wiring 已建立并在本机 exact tuple 下通过，正式 reviewed CI attestation 尚未产生
- 当前授权范围：TASK-004 已由 Specification v1.1.12、ADR-0006、accepted implementation contract 和 Plan start record 授权为 `IN_PROGRESS`；实现与本地 gate 已完成但正式 reviewed CI attestation 尚未产生，只能修改该 record 的 exact scope；TASK-003 及后续 task 仍未授权，等待 TASK-004 正式完成并提供 durable Library owner/lock context

## 工作规则

- 首次接管时先检查项目和文档，不直接修改实现代码。
- `IMPLEMENTATION_REVIEW.md` 仍有适用于当前 task 的 `BLOCKER` 或 plan 标记 `BLOCKED` 时，只能执行证据、决策和文档工作，不得初始化实现代码。
- 以 `IMPLEMENTATION_SPEC.md` 中的 `CONFIRMED` 条目为强约束。
- 架构冲突或强约束变更必须先记录到 `DECISIONS.md`，必要时建立 ADR。
- 每个实施阶段开始前确认前置决策，结束后运行相应验证命令并更新计划状态。
- 不编造标记为 `OPEN` 或 `TBD` 的选择和指标。
