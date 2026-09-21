---
title: "macOS / Ubuntu 双版本开发计划"
document_role: "Accepted development direction; implementation gates pending"
status: "MACOS_FIRST_UBUNTU_DEFERRED_NO_IMPLEMENTATION_AUTHORITY"
version: "0.1.6"
date: "2026-09-20"
decision: "ADR-0019"
---

# macOS / Ubuntu 双版本开发计划

## 1. 目标和本轮边界

CURRENT_PROJECT_NEXT_ACTION: DRAFT_REVIEWED_EXECUTION_PROFILE_GATE

Current scoped completion (2026-09-21): BROKER_FOUNDATION v0.1.1 is DONE.
PR #18 head 37b11896a339158807e050f007aea54fc7600349 passed run 35548864113;
merged main 88d1ac06fa4a9bcc4c2877cf1c67dbe68f01fc6c passed run 35550089829.
Implementation and product authority are NONE. Evidence and limits:
docs/proposals/BROKER-FOUNDATION-GATE-PROPOSAL.md §17 and the scoped lifecycle ledger.
Next is drafting the reviewed execution-profile qualification gate only, not
launching plugins or implementing a runtime. Parent TASK-013 is NOT_CLAIMED;
strict TASK-012 remains BLOCKED. Earlier draft/start/local-only/no-merge statements
below are historical and superseded only for this completed pure scope.
No production capability, migration, Ubuntu work or renewed R0/VM research follows.

Latest completion: reviewed plan §5 pure foundation is DONE with exact PR/main
evidence in its §9. Implementation and product authority are NONE. Next is the
non-executing BROKER_FOUNDATION gate draft; older routing below is historical.
No R0/VM experiment or strict-backend acceptance is authorized.

Current routing amendment / 当前路线更新：ADR-0021 已接受审核准入原生插件方向。
纯准入基础已经完成；下一步起草 BROKER_FOUNDATION 的有界非执行合同 gate；
不再要求选择 VM 或重复原生硬内存探针。reviewed profile 明确接受剩余内存 DoS
风险，但不放宽文件/网络/IPC、身份、Broker/凭据与生命周期边界。产品权限仍为 NONE。
本文件下文的研究结论、候选与当时下一步属于保留的历史/严格 profile 范围，
不能覆盖新的方向，也不能作为 reviewed runtime 已合格的证据。


同一仓库、同一业务核心，分别验收和交付 macOS 版与 Ubuntu 版。
按用户最新决定，先完成 macOS 版，再启动 Ubuntu；当前不并行开发两版。
Ubuntu 版本选择、主机接管、平台适配、backend 取证和产品 CI 均搁置。
未来 Ubuntu 直接在真实 Ubuntu 环境
开发，不以虚拟机、容器或远程执行替代实际平台资格验证。Windows 不在本轮范围。

本轮只接受组织方式与实施顺序，不启用任何产品能力，不修改实现、协议字节、
数据库迁移、工具链或 CI。后续按一次明确的工作包 gate 授权修改必要的已有代码，
无需为旧任务逐个重新开工；但必须保留其功能契约和相关回归证据。
本计划不是新增稳定 TASK 编号，不能作为 production spawn/sandbox/Admin 的启动记录。

## 2. 当前事实与差异

| 范围 | macOS | Ubuntu | 处置 |
|---|---|---|---|
| 已完成 TASK-001 至 TASK-011 的交付 | 保留原精确 commit/run 和范围 | 尚无 native 支持证据 | `EXPECTED_GAP`，不把旧任务 DONE 外推为 Ubuntu DONE |
| 本地升级后的系统 | 有 macOS 27 开发兼容性检查；不是新 formal/sandbox 认证 | 版本、架构、内核和文件系统未确认 | `UNKNOWN`，只读接管后再定支持 tuple |
| filesystem / IPC / 构建 | 现有 arm64 macOS 实现 | 需适配与验收 | `EXPECTED_GAP`，不是直接换 target 编译即可 |
| TASK-012 生产 sandbox | 候选草案仍受 hard-memory 阻塞 | 独立 gate 未建立 | 分别阻塞；不推断 Linux 必然可行 |
| Admin / Broker / Credential / 真实 Provider | 后续未授权 | 后续未授权 | 保留安全依赖与各版授权边界 |

仓库依据：`crates/mengxia-platform-fs/build.rs` 的 HOST/TARGET 固定 macOS，
`src/lib.rs` 直接使用 macOS FFI；现有 APFS/ACL/durability 合同不可直接外推。
`crates/mengxia-plugin-package/src/manifest.rs` 和
`schemas/plugin/manifest-v1.schema.json` 只接受 `aarch64-apple-darwin`。
TASK-011 的 caller-supplied stream/session 可以复用，但不包含生产进程权限。
CI 使用 Ubuntu runner 做分类等工作，不代表 Ubuntu 产品已经被构建和验证。

## 3. 共用与分离

| 共用且不得分叉 | 各版独立实现/证明 |
|---|---|
| domain、application、业务状态机、错误语义、稳定 AC/TEST 义务 | 文件句柄/path race、ownership/ACL、根目录身份、锁、原子 no-clobber、durability |
| SQLite schema、migration 历史、协议与包格式的版本演进 | SQLite/native 编译工具证据、文件系统持久性和崩溃恢复 |
| 授权策略、SandboxEvidence 各维度语义、资源预算和生命周期合同 | IPC peer identity、进程/镜像绑定、sandbox、清理、后续 secret store/Admin |
| 通用测试场景、hostile fixture 行为、观测性语义 | 平台负向用例、真实第二 UID、native hostile suite、发布产物与支持 tuple |

优先在既有基础设施 crate 的私有平台模块中分离差异；不预先复制整个 crate 树，
不在业务层散布 OS 判断。具体拆分以依赖审查为准，不为尚未确定的后端制造通用框架。
共享 schema 不等于支持跨系统直接复制/打开 Library：backend identity、路径、
大小写与本地 ownership 仍须验证；跨平台导入/迁移需要另行明确的非破坏性流程。

## 4. 工作包与启动顺序

以下名称仅为工作包标签，非任务状态或实现授权。保留现有稳定 task 编号。

### 当前：完成 macOS 版

ADR-0020 已接受先完成 macOS 内置功能，第三方 Native 安装/激活/执行延后。
后续用户已将有界 R0-B 原生研究设为当前优先项；批次结果及下一步统一见
MACOS-NATIVE-SUPPORT-DEVELOPMENT-PLAN.md §9，不把已结束批次当成新的实验授权；
规范 §0.7 的非执行性基础在技术依赖上仍不等待
原 TASK-012 Native 候选。内置执行仍有自己的 profile/资源/隔离验证门禁，
不能借用禁用第三方来伪造通过。这里的“完成 macOS 版”指已接受的 macOS 功能范围及其适用
TASK-023 验收/release gate 完成，不是仅完成 TASK-012，也不把未验收 preview
算作完成。不能为提前进入 Ubuntu 而删减 macOS 功能或放松安全要求。

当前只保留必要的平台边界，不提前为 Linux 改造代码、扩充 target/schema、
加入 Ubuntu 产品 CI 或多平台状态工具。下面 A–E 的 Ubuntu 内容是后续路线，
不是当前待执行任务；macOS 完成后再以新的接管与启动 gate 启动。

### A. Ubuntu 只读接管（搁置，macOS 完成后）

在用户的 Ubuntu 主机读取版本/架构、内核、工具是否存在及其版本、workspace 与
拟用 Library 的文件系统/挂载参数、账户权限和可用 cgroup/安全机制。
可使用 `uname -r`、`uname -m`、读取 `/etc/os-release`、`id`，以及对用户明确
指定路径的 `findmnt -T`。不存在的命令记为 UNKNOWN，不自动安装；未选定 Library
路径不擅自创建。仅收集必要字段，不导出环境变量、秘密或全量系统配置。

检查 cgroup v2 的控制器及实际用户委托权限，而非仅看目录存在；记录内核能力与
发行版策略的差异。只读阶段不运行逃逸/压力实验、不启用 user namespace、
不执行 sudo、不修改 service/ACL/cgroup 或系统安全策略。

出口：真实 Ubuntu intake + 候选支持 tuple + 缺口/风险清单。当前均待确认，
不预设发行版版本、CPU 架构、ext4/XFS/Btrfs 或 cgroup 委托可用。

### B. Ubuntu foundation 最小垂直切片（搁置）

先制定一个 bounded foundation gate：列出具体文件、现有 task 的复用合同、
新增/复用稳定测试义务、平台证据记录与必要 CI 适配；接受后才修改代码。
gate 必须覆盖同一变更内需要调整的构建脚本、fixture、测试和文档，避免实施中
因遗漏兼容性文件再次停工。不得用目录级无限授权代替具体影响分析。

建议实现顺序：平台构建选择与 native 证据 → filesystem ownership/identity/lock/
durability → SQLite/CAS/recovery → 普通 Client peer identity/endpoint → CLI/daemon
已实现读写流程。目标是把已有产品功能带到 Ubuntu，不先扩大插件能力。

验收至少覆盖：冷构建及供应链、真实第二 UID、符号链接/路径替换、ACL/mode、
锁竞争、exact-case no-clobber、崩溃恢复/WAL/corruption、bounded cancellation，
以及现有 ingest/read/verify/materialize/creative ledger E2E。平台义务可以用等价
Linux 机制满足，但不能将整个安全测试标为不适用。macOS 完整相关回归同时保持通过。
Linux 下不得以空 build script、stub 成功或跳过权限检查获得“可编译/可用”结论。

### C. 平台 target 与包兼容性（Ubuntu 启动后）

Ubuntu foundation 稳定后，为目标架构的 Plugin package 制定版本化兼容变更。
保留 manifest v1 的历史字节、digest 和测试 fixture；新增版本/明确的兼容读取
合同经过 gate 决定，不直接放宽冻结的 v1 target 常量。错误平台、未知 target、
旧版不支持新格式必须明确拒绝；新增 target 不能隐含授予安装、激活或执行权限。
本包可与 sandbox 可行性取证并行，但必须在实际 Linux Plugin 启动前完成。

### D. 延后的第三方 Native backend：仅在重开条件满足后分别验收

- macOS：现有 `TASK-012-GATE-PROPOSAL.md` 仅约束已延期的 Native candidate。
  只有出现新机制或可重复证据才考虑重开；届时仍须解决 hard-memory、镜像绑定、
  same-image re-exec 和 lifecycle 的真实证据，不能把该候选当成已合格的内置后端。
  当前已由用户重开有界 R0-B 取证，不直接实施该旧草案，也不转向 TASK-015。
- Ubuntu：当前搁置，未来 B 完成后接受独立 backend gate；可在 A 后另行限定可行性取证。
  必须证明 filesystem/network/process/IPC/resources 的完整强制隔离、精确镜像绑定、
  无不受限执行窗口、限额/终止/回收及 hostile matrix。不能凭机制清单直接接受。

Linux 的 cgroup v2 可作为资源限制候选，Landlock 等机制可作为访问控制候选，
都不是完整 sandbox 的替代物。具体组合、ABI、委托/部署权限、swap/线程/子进程
计费、竞态与宿主保护必须在真实主机上验证。官方依据：
[cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)、
[Landlock](https://docs.kernel.org/userspace-api/landlock.html)。这些资料说明机制，
不证明用户主机已具备能力；任何缺项都保持拒绝启动，不降级成 RSS 轮询或普通进程。

### E. 各版后续业务功能

当前按 ADR-0020 和 Specification §0.7 的分阶段依赖准备内置业务，不以本节
延后的第三方 Native backend 为纯基础的前置条件；各阶段仍须自己的启动/验收 gate，
也不等待 Ubuntu 的实现或验收。未来 Ubuntu 复用共享业务，但 foundation/package/
sandbox/Admin 等平台义务仍须独立通过，不能拿 macOS DONE 直接开启 Linux 入口。
共享接口、迁移序号和协议版本统一维护；移植时不复制另一套业务/schema 历史。

## 5. 状态、证据与 CI

不重编号旧 TASK/AC/TEST，不抹掉已完成交付。新证据必须记录 task + edition +
精确 commit/tree + OS/arch/kernel/filesystem/tool/backend tuple + 单项结果和 run。
同一稳定 TEST ID 可以有多个平台结果，不能仅因 ID 相同合并为一份 PASS。
通用断言可复用测试实现；OS 行为必须来自真实目标环境。
当前 task 表的 DONE 明确对应已接受的 macOS 范围，可满足 macOS 后继任务和
TASK-023，不等待 Ubuntu；Ubuntu 以后另加适配验收，不撤销 macOS 历史结项。
不得外推的是 Ubuntu/双版本整体完成，不是禁止当前 macOS task 结项。

未来 Ubuntu 首次平台实现 gate 必须同步机器记录/校验器/CI 的 edition 支持，并加入负向测试：
缺平台证据、错 tuple、适用测试 skipped、拿 macOS 结果代替 Ubuntu 都应失败。
现有 lifecycle TOML 不擅自加未支持字段。该机制完成前，文本写“scoped DONE”
不能用来绕过当前 aggregate gate，也不能要求手改旧 Rust 状态常量维持假通过。
这不是当前 macOS task 的新增 gate；macOS 继续使用现有单平台证据体系。

保留现有 macOS required checks、Formal、真实第二 UID、供应链与 CodeQL。
Ubuntu 产品 CI 当前不新增；未来启动时以明确未认证的 bootstrap 验证接入，宣称支持前必须形成强制的
Ubuntu native/formal/第二 UID 与精确 merged-main 验证，更新聚合 merge gate，
不能永久 allow-failure。相关 CodeQL/供应链覆盖要核对新增 Linux 路径与依赖。
现有 ADR-0015 的去重原则继续适用，不重复执行同一目标只为归属多个稳定 ID。

Ubuntu 支持进入已验收范围后，共享代码/协议/schema/依赖/CI/平台边界的改动必须
验证两版；只有经审查证明平台局部性的改动才可缩小范围，分类不明跑全量。
macOS 专属 FFI 证明无需在 Linux 伪造，但其可移植安全义务必须由 Linux 测试覆盖。
任一失败若涉及共享合同，两版相关发布均需评估；不能机械当成“另一平台故障”。

## 6. 发布及安全底线

每版发布列明支持 tuple、已启用功能、未支持功能与各自验收证据。版本号可沿用
共享代码版本，加平台产物标签；具体签名/打包方式留给各版 release gate。
首版先完成并交付 macOS，但不能把单版/受限 preview 宣称为双版本整体 V1 完成。
核心安全要求不变：第三方 Native 无完整隔离则拒绝；Admin、Credential、真实
Provider egress 等必须满足各自 gate；客户端可见能力缺失应明确、稳定且无副作用。
macOS 主机不自动连接 Ubuntu 代执行，不增加远程服务或网络暴露。

升级 OS/内核/工具后仍按 ADR-0016 区分开发兼容与 formal/runtime qualification，
新 tuple 不继承旧 sandbox 证明。回退应回退待发布代码/能力开关，不自动降级系统、
关闭系统保护或篡改历史迁移。安全修复和 shared-contract 回归按影响面处理。

## 7. 完成条件与避免返工

本轮完成：架构决策、canonical 依赖解释、macOS 优先计划、草案范围和接管说明
保持一致，通过文档门禁；不声称实现完成。
下一步按用户后续优先级推进 R0-B 原生取证；PLAN_FOUNDATION 保留为独立后续工作，
不新增其对 Native 的技术依赖。也不索取 Ubuntu 版本/架构或提前细化
Linux 实现。Ubuntu 路线留待 macOS 完成后重新接管。
仅当真实新证据改变安全合同、
依赖或授权时重开相应 gate；非阻塞编辑和措辞优化不挡实际开发。
