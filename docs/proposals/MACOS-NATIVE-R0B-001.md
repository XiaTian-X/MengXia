---
title: "R0-B 第一批：Seatbelt 与资源组合反例"
version: "0.1.0"
status: "FIRST_BATCH_OBSERVED_RESOURCE_GAP_REMAINS_NO_PRODUCT_AUTHORITY"
date: "2026-09-20"
---

# R0-B 第一批实验

CURRENT_PROJECT_NEXT_ACTION: COMPLETE_BROKER_FOUNDATION

Current scoped start (2026-09-21): BROKER_FOUNDATION v0.1.1 is accepted and
IN_PROGRESS; implementation authority is BROKER_FOUNDATION_ONLY, product authority
is NONE. Exact files/contracts: docs/proposals/BROKER-FOUNDATION-GATE-PROPOSAL.md
§9 and §15; lifecycle evidence is in docs/spec/task-lifecycle-records.toml.
Complete the pure comparison/audit and accounting tests; no IO, launch or durable
lease authority. Earlier draft-only/no-active-authority routing below is historical
and superseded only for this bounded scope; completed-task and strict-profile
evidence remains unchanged. No parent-task completion or production start follows.

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


## 1. 授权与假设

用户要求优先解决原生问题并明确启动。DECISIONS 已记录排期修正；ADR-0020 的
首版范围、安全约束、依赖仍有效。本批不启动 TASK-012 生产实现或 TASK-015。
研究候选是最小原生进程自行安装 Seatbelt，不是 Apple 扩展。后者的签名/注册
条件不影响本批；没有选择两套生产后端。

假设 H1：当前 VAS + 增量的 RLIMIT_AS 能否阻止**已有映射**的驻留内存增长？
假设 H2：仅两个预创建、不可新增/重命名的可写文件槽，加每文件 64 KiB 限额，
能否得到这个封闭夹具的 128 KiB 逻辑数据边界？这不等于物理磁盘/元数据总配额。
先检验机制与反例；镜像 custody、完整 IPC/网络/线程/内核记账和真实媒体兼容性
仍待验证，不会把 deny-default 的名字当作全维度证明。

## 2. 精确范围与预算

新建 `scripts/native-research/r0b-001.c`、`scripts/native-research/r0b-001.sh`
及本文；只读复用 `controller.c` 的进程、期限、清理函数，不改变 R0-A 七个文件。
路线同步范围为 AGENTS、五份 canonical 文档、已有路线/候选文档、ADR-0020、
document_traceability 与版本台账。无需新的产品 ADR、依赖、稳定 TEST 或迁移。

每批 CONTROL/SANDBOX/RESERVED_MEMORY 各三次，串行一个自有子进程；每次 5 秒
自身 alarm，外部 10 秒及 TERM/KILL 清理，整批 120 秒；编译单独 120 秒。
每次最多保留一个 8 MiB 映射、逐页触碰最多 8 MiB，额外探测映射最多 4 MiB且不触页；
每个文件槽最多尝试写 96 KiB，第三个新文件测试最多写零字节；输出各 32 KiB。
父进程新建三个私有文件（slot-a、slot-b、canary），控制组可新建一个零字节第三槽；
无真实 Library/秘密。文件名变更只在私有目录内进行且控制组随即恢复原名。
文件控制组确认正常访问，沙箱组检验拒绝读 canary、新建第三槽、重命名和 fork。
fork 如意外成功，仅产生一个立即退出并回收的自有子进程；绝不循环生成。
不接触网络，不运行外部工具/插件/媒体，不注册、签名、挂载或变更全局设置。

## 3. 构建与观察

runner 使用已观察 SDK、固定 clang 参数、只读源码快照及干净环境。C 主程序
没有任意 executable/profile 参数；仅能 spawn 同一程序的三个固定 case。
复用现有 controller 的白名单 FD spawn、单调时钟、输出上限及清理函数；程序
只允许可信有限探针，不能用来执行 hostile binary。所有产物留在新建临时根。
弃用的 sandbox_init 调用只在单个调用点抑制弃用诊断，并明确保留支持性风险。
沙箱只施加于一次性子进程，不影响调用者和系统。初始化失败则停止该观察，无回退。

原始 stdout/stderr、父进程退出/回收/文件大小观察、源码/二进制/工具 hash、tuple
及运行返回值保存；独立命名 R0B001，不送入 R0-A schema-2 verifier冒充其证据。
父进程检查退出、输出上限和独立文件大小；原始固定字段由本轮另外进行的
只读语义核查复验，不声称已有独立 schema verifier。H1 反例不是后端合格。
异常或缺失证据停止后续启动；退出未确认保留自有 controller 回收，不靠旧 PID续杀。

## 4. 判定与后续

本批最多形成局部 MECHANISM_OBSERVED / COUNTEREXAMPLE / INCONCLUSIVE。
任何结果都不构造 SandboxEvidence、不批准 R1、不启用第三方或生产内置执行。
H1 被否定时，不继续把相对 AS 当硬物理内存；先找覆盖现有保留区/共享/内核路径
的实际系统机制。H2 成立也仅支持固定槽模型，再研究所需真实工作流能否使用它。
未证明不存在其他方案，不做 macOS Native 全局 NO-GO。

来源：重读 [Chromium 最小启动器设计](https://chromium.googlesource.com/chromium/src/+/main/sandbox/mac/seatbelt_sandbox_design.md)
与本机 SDK 的 sandbox/resource 头文件。借鉴先限制后加载，不复制其产品安全结论；
该文档明确指出 SBPL 非正式文档化，接口稳定性仍是产品选择前的独立问题。

## 5. 实际结果

### 5.1 最终批次

最终代码的九次观察保存在 `/tmp/mengxia-r0b001.cfXiJ9iA`；CONTROL、SANDBOX、
RESERVED_MEMORY 各三次均正常退出、直接子进程全部确认回收、stderr 空。
当前 tuple 仍为 arm64 macOS 27.0 / 26A428、Xcode 27.0 / 27A266a、SDK 27.0。
这是未提交工作树的临时开发证据，不是正式 attestation；临时目录可能被系统清理。
源文件入库后可按 `sh scripts/native-research/r0b-001.sh` 重现，新的结果独立保存。

| 观察 | CONTROL（3/3） | SANDBOX（3/3） |
|---|---|---|
| 两个槽各尝试写 96 KiB | 各保留 65536 字节，EFBIG=27 | 同左，父进程 fstat 独立确认 |
| 读私有 canary | 成功 | EPERM=1 |
| 新建第三槽 / 重命名槽 | 成功，第三槽零字节 / 原名恢复 | EPERM=1 / EPERM=1 |
| 一次 fork | 子进程立即退出并回收 | EPERM=1 |

结论：本夹具观察到“固定允许槽 + 每文件限额”的组合机制。它只限制这里测到的
逻辑文件长度，不证明物理块/元数据/累计 I/O、文件映射写入、共享服务或真实媒体
程序的总资源资格；未实测路径不得据此补成 ENFORCED。也没有测试网络连接或
完整 Mach/IPC、线程和运行镜像身份，仅有 deny-default 不能代替这些负向测试。

内存组安装同一 Seatbelt 后，将 RLIMIT_AS 设为当前 VAS + 1 MiB；新申请
4 MiB 映射三次均以 ENOMEM=12 拒绝。随后逐字节填充预先创建的自身 8 MiB
映射，等待 20 ms 再读自身统计。VAS 三次不变，物理 footprint 增量分别为
8,388,608 / 8,405,016 / 8,388,608 字节，均超过 1 MiB。

因此 H1 被局部反例否定：相对 AS 能限制被测新映射，但不能单独保证既有映射的
驻留/footprint 增量也受 headroom 约束。本实验没有触碰任何系统原有保留区，
没有证明实际 worker 必然预先映射同样区域；未来若依靠排除这类映射，必须证明
完整启动/依赖/共享/可达内存路径受约束，而不能仅删掉这个 case。它不否定
RLIMIT_AS 的地址限制作用，也不证明不存在其他可行原生方案。

### 5.2 过程、来源与限制

首次 runner 因 mkdir 的系统路径错误在编译前停止，未启动探针；已修正。
初始稀疏触页批次 `/tmp/mengxia-r0b001.30EzfQoi` 仅观测到 16 KiB 的即时
footprint 增长，未证明 H1 反例，其原因未判定。保留该批次，不择优删除；随后
采用预算内的逐字节填充、20 ms 后采样，形成上述新方法的三个一致反例。
不能把两个不同写入/采样方法当作同一组重复实验或统计置信度。

最终源码快照和产物 SHA-256：

- `controller.c`：`ab0bb2ec9494e36dbaf3927ab959561b61cad4dc94b4530e0cf6a6204e3278d8`
- `r0b-001.c`：`1a343709853dda856813655693daf0ec457b82a08199720d722cae4f4efa34cd`
- `r0b-001.sh`：`13c41fe33d8697c079b8eb791efd5f5ba62eb04cdf1be654c8dcaab20d3f83a3`
- probe：`53ebb7bb4a5528227ebb3cbb69ebc40366ac79f164851448c3be037f2aef1ae2`

原始 `controller.stdout`、各 case stdout/stderr、toolchain.txt、build.txt、host.txt
与 sha256.txt 在同一目录。本批复用但未改 R0-A controller，不覆盖 schema-2
证据。`controller.stdout` SHA-256 为
`34f74581c52ad960a3ebaddef096972e5f1b9af7349e1069ae4d5bf32fc6045d`；
`sha256.txt` SHA-256 为
`36354d1eabdebd01bd02c2268992842982c6bdd7d7bd411dd741c0c0b720f13c`。

交付验证：9 个原始输出逐项核对控制组/拒绝 errno、槽大小、VAS/footprint 差值，
与本节结论一致；9 个直接子进程正常退出并回收。clang 静态分析无诊断，Shell
语法、Rust fmt 和空白检查通过；`verify-repository.sh docs` 的 27 项检查通过。
初次文档检查发现升版后遗漏的 Plan 内嵌版本与输出合同原句，已同步修正，未
删除其语义约束。原 R0-A schema-2 证据重新只读验证通过（36 expected，0 未决，
0 不稳定）；没有为本批重跑 R0-A 资源探针或改写其源码/证据。
没有 hosted CI、Formal、真实第二 UID、签名扩展或产品测试，不授予任何产品资格。

### 5.3 出口

首批研究完成，有限实施授权结束；当前优先级仍为原生可行性，不转向 TASK-015。
下一项证据问题收敛为**覆盖已有映射和共享/内核路径的内存强制机制**；在其成立前，
不扩建完整 Seatbelt 后端、SDK 或重开 TASK-012 生产实施。固定槽方案保留为
逻辑输出约束候选，兼容性和完整总量覆盖待验。总体仍为 INCONCLUSIVE，
仅“AS headroom 单独约束真实内存增量”的具体论证已被反例否定。
