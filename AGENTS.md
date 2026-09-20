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

CURRENT_PROJECT_NEXT_ACTION: DRAFT_BROKER_FOUNDATION_GATE

Foundation completion (2026-09-20): the pure reviewed-native foundation is DONE
with PR #16 and exact merged-main evidence in reviewed plan §9 and the scoped
lifecycle ledger. Its implementation authority is revoked to NONE; product authority
remains NONE. Old strict TASK-012 stays BLOCKED and its parent is not completed.
Next is drafting the non-executing BROKER_FOUNDATION gate, not implementing Broker,
launching plugins or repeating R0/VM experiments. Earlier start/validation/routing
statements below are historical and cannot grant current implementation authority.

ADR-0021 已接受“审核准入原生插件”方向：独立原生进程、逐版本/精确产物与依赖
闭包审核、最小权限与持续撤销。只对 reviewed profile 明确接受未建立硬物理内存
限制的宿主 DoS 剩余风险；文件/网络/IPC、可执行身份、Broker/凭据、生命周期等
边界不放宽，不等于 SANDBOX_ONLY 或无限权限 TRUSTED_NATIVE。

纯准入判定基础已按 `docs/proposals/REVIEWED-NATIVE-PLUGIN-DEVELOPMENT-PLAN.md` §9
完成，scope 为 DONE，实施与产品 authority 均为 NONE。下一步仅起草 BROKER_FOUNDATION gate。
用户允许必要时重构已实现代码，但须限定必要接口/测试、保留功能/数据/协议兼容
及回归证据；不要求整体重写，不自动授权生产执行或外部插件运行。

R0/R0-B 证据保持原结论；原严格 TASK-012 候选继续 BLOCKED，新 profile 不继承
其全维度 ENFORCED 要求或 PASS。此前 VM 比较和反复硬内存研究不再是当前动作。
Ubuntu 继续延后，内置集成先行；纯 scoped 完成不能冒充父任务 DONE。

- 项目阶段：Implementation / Phase 3A Plugin private protocol；TASK-001、TASK-002、TASK-004、TASK-003、TASK-005、TASK-006、TASK-007、TASK-008、TASK-009、TASK-010 foundation、TASK-011 complete
- 实现范围：V1 / MVP
- 已接受双版本方向：ADR-0019；macOS / Ubuntu 共用业务核心、独立验收。先完成 macOS，再启动 Ubuntu；Ubuntu 版本选择、接管、开发和产品 CI 搁置。ADR-0020 接受 macOS 内置集成优先；ADR-0021 接受后续审核准入第三方方向，产品安装/激活/执行仍保持禁用；TASK-015 草案保留为独立后续工作。作用域依赖以规范 §0.7 为准；内置执行仍须独立 profile 验证，凭据/Broker/Rights/Admin 等适用安全门禁不降低。既有 DONE 不表示 Linux 支持；当前产品实现权限仍为 NONE。
- 当前仓库：TASK-001/TASK-002/TASK-004/TASK-003/TASK-005/TASK-006/TASK-007/TASK-008/TASK-009、TASK-010 foundation、TASK-011 已完成；原 TASK-009 implementation head `fa7a0047c95c8b8eba12e859284223a1a78f51e2` 与 reviewed run `34552988098` 保留为交付证据，completion-gate correction `decfc82fadfd2a26221007fc67a5bc189845985d` 由 reviewed run `34554608874` 覆盖，post-completion ledger-validation correction `c3fa74a` 由 exact descendant head `05bce461b18fad6da77efe085913c1142c98c9e6` 的 reviewed run `34559210695` 覆盖；MAINT-001 已由 PR `#1` 合并为 `2dd5bcb76a8eb6b804ef55b10d78dd715bdaebe4`，最终 PR run `34565503807` 与 merged-main run `34566194911` 为正式证据；TASK-010 foundation implementation head `e2311ed1dea992ce85db2547a1af799d0d8cf045` 由 PR `#4` reviewed run `34667611801` 覆盖；TASK-011 PR head `fc817200d2a60c88c4d16cc5e4c60a6be2dd1cbf` 由 run `34761111053` 覆盖，合并提交 `8416e01335e4fb8ff58e3381cf888fbcf69b9005` 由 main run `34761648787` 覆盖；TASK-011 session correction head `96bc2224030fd054f3f272a4ee6ee5a179766ec6` 由 PR `#14` run `34808311370` 覆盖，合并提交 `b7ce104750a5aff54ee7576d2adb0ceb0c1fa3d2` 由 main run `34808937770` 覆盖，PR/main CodeQL runs `34808310341` / `34808937066` 均通过
- MAINT-002 CI 维护已完成：PR `#5` run `34676854969` 与合并提交 `19e2e613728c2a2c11f6c3dfc185b04e3a625316` 的 main run `34677363307` 均已核验；执行规则见 ADR-0015，详细证据见 MAINT-002 规划 §12。
- MAINT-003 工具链维护已完成：PR `#8` run `34731852394` 与合并提交 `7b6fa4f17373ab94aa86057d8ddc9b4d23c23b8c` 的 main run `34732388943` 均已核验；详细证据及覆盖边界见 MAINT-003 规划 §14。
- Build-host ACL 安全修复已完成：PR `#10` run `34739311995` 与合并提交 `632a2aac725001332fb8541806abe9cdcfcc65ee` 的 main run `34739722410` 均已核验；精确证据见 IMPLEMENTATION_PLAN.md 的 ACL correction completion。
- 当前实施与产品权限均为 `NONE`；纯准入基础、TASK-011 authority 均已撤销。下一步仅起草 BROKER_FOUNDATION gate；TASK-012+、root rebind、Admin、Plugin installation/activation、production spawn/kill/sandbox/Broker、Credential、Rights 与 destructive behavior 仍未授权

MAINT002_DECISION: ADR-0015
当前 task 表的 DONE 仅表示已接受的 macOS 范围完成，可以满足 macOS 后续任务和 TASK-023；不等待 Ubuntu，也不声称双版本完成。TASK-012 必须区分前置可行性、封闭测试资格验证与最终生产资格验证；完整 hostile-suite 不是编写其实现的前置条件，但硬内存可行性仍未通过；该阻塞针对严格 profile，reviewed profile 按 ADR-0021 单独验收，产品 authority 仍为 NONE，已完成的纯基础不授予后续实施权限。

MAINT003_DECISION: ADR-0016
MAINT003_LIFECYCLE: DONE
MAINT003_IMPLEMENTATION_AUTHORITY: NONE
MAINT003_PRODUCT_AUTHORITY: NONE

2026-09-13 MAINT-003 已通过 reviewed PR 与 merged-main 验收，历史启动授权已撤销。
ADR-0016 的工具链规则继续有效；工具版本与产品范围未改变，常驻修复代理未启用。

MAINT002_LIFECYCLE: DONE
MAINT002_IMPLEMENTATION_AUTHORITY: NONE
MAINT002_PRODUCT_AUTHORITY: NONE

MAINT001_DECISION: ADR-0013
MAINT001_LIFECYCLE: DONE
MAINT001_IMPLEMENTATION_AUTHORITY: NONE
MAINT001_PRODUCT_AUTHORITY: NONE
MAINT001_FORBIDDEN: CARGO_LOCK_TOOL_VERSION; THIRD_PARTY; PROTO_ARTIFACT; MIGRATION; PRODUCT_RUNTIME; TASK_010_PLUS

TASK010_CANONICAL_GATE: ACCEPTED
TASK010_LIFECYCLE: DONE
TASK010_IMPLEMENTATION_AUTHORITY: NONE
TASK010_PROPOSAL: docs/proposals/TASK-010-GATE-PROPOSAL.md
TASK010_PROPOSAL_VERSION: 0.2.3
TASK010_DECISION: ADR-0014 ACCEPTED
TASK010_FOUNDATION_ACCEPTANCE: AC-098; AC-099; AC-100
TASK010_FORBIDDEN: FILESYSTEM; STORE; MIGRATION; APP; PORTS; PROTO; CLI; DAEMON; ADMIN; INSTALL; GRANT; REVOCATION; ACTIVATION; EXECUTION; TASK_011_PLUS

TASK011_CANONICAL_GATE: ACCEPTED
TASK011_LIFECYCLE: DONE
TASK011_IMPLEMENTATION_AUTHORITY: NONE
TASK011_PROPOSAL: docs/proposals/TASK-011-GATE-PROPOSAL.md
TASK011_PROPOSAL_VERSION: 0.1.2
TASK011_DECISION: ADR-0017 ACCEPTED
TASK011_ACCEPTANCE: AC-101; AC-102; AC-103
TASK011_FORBIDDEN: INSTALL; GRANT; REVOCATION; ACTIVATION; EXECUTABLE_CUSTODY; PRODUCTION_SPAWN; KILL; SANDBOX; BROKER; CORE; ADMIN; DB; CAS; PERSISTENCE; MIGRATION; CLI; DAEMON; TASK_012_PLUS

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

TASK008_CANONICAL_GATE: ACCEPTED
TASK008_SPECIFICATION_VERSION: 1.1.30
TASK008_LIFECYCLE: DONE
TASK008_IMPLEMENTATION_AUTHORITY: NONE
TASK008_PROPOSAL: docs/proposals/TASK-008-GATE-PROPOSAL.md

TASK009_CANONICAL_GATE: ACCEPTED
TASK009_SPECIFICATION_VERSION: 1.1.34
TASK009_LIFECYCLE: DONE
TASK009_IMPLEMENTATION_AUTHORITY: NONE
TASK009_PROPOSAL: docs/proposals/TASK-009-GATE-PROPOSAL.md

## 工作规则

内置路线按规范 §0.7：启动前确定分阶段验收规则/文件/测试，首次实施中建立
验证器，记录或消费完成状态前必须通过。BROKER_FOUNDATION 仅为纯内存合同；
执行资格验证后才进入 BROKER_PERSISTENCE（0003/0004），真实 Run/租约/审计组合
由 RUN_INTEGRATION（0005）验收。不得跳迁移、伪造产品 Run 或用纯合同替代持久化。

2026-09-13 独立 ACL 修复历史启动：用户曾授权 BUILD_ACL_CORRECTION_ONLY，范围见
DECISIONS.md 的 Build-host ACL correction start；现经 reviewed PR/main 验收，
该临时授权已撤销。本修复未升级工具、未改产品代码，未授权 TASK-011+。

- 首次接管时先检查项目和文档，不直接修改实现代码。
- `IMPLEMENTATION_REVIEW.md` 仍有适用于当前 task 的 `BLOCKER` 或 plan 标记 `BLOCKED` 时，只能执行证据、决策和文档工作，不得初始化实现代码。
- 以 `IMPLEMENTATION_SPEC.md` 中的 `CONFIRMED` 条目为强约束。
- 架构冲突或强约束变更必须先记录到 `DECISIONS.md`，必要时建立 ADR。
- 每个实施阶段开始前确认前置决策，结束后运行相应验证命令并更新计划状态。
- 不编造标记为 `OPEN` 或 `TBD` 的选择和指标。
