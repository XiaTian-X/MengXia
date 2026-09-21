---
title: "R0-B 第二批：替代内存限额入口核查"
version: "0.1.0"
status: "OBSERVED_MEMORY_ENTRYPOINT_GAPS_NO_PRODUCT_AUTHORITY"
date: "2026-09-20"
---

# R0-B 第二批

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


## 1. 问题与范围

本批接续 R0B001，不重跑 R0-A，不修改安全要求或生产实现。
H3：本机普通用户能否通过另一个入口 `memorystatus_control` 设置自身物理
footprint 限额？H4：`RLIMIT_DATA` 是否能补足 AS 对既有映射增长的缺口？

用户继续请求与 DECISIONS 的 second bounded memory experiment 记录授权
三个新文件以及路线/证据/版本/文档测试同步。旧研究源码和证据只读。
只运行自身编译的固定探针，不接受用户程序、PID、参数或策略输入。

两个固定 case 各三次，串行一个子进程；自身 alarm 5 秒、父监督 10 秒、批次
120 秒、编译 120 秒，stdout/stderr 各 32 KiB。复用已审查 controller 的
关闭继承 FD、期限、取消、TERM/KILL/wait 清理。临时私有目录保留证据。
MEMLIMIT 只请求自身 active/inactive 各 128 MiB、fatal 属性，不分配负载；
先确认非 root 且当前 footprint 小于 32 MiB。请求成功也不测试耗尽或宣布合格。
DATA 先试 64 MiB DATA 限额；若成功则停止该 case，避免继续降低/抬高已生效
硬限额。若失败则记录 errno，使用自有 8 MiB 映射与当前 VAS + 1 MiB DATA
限额，尝试额外 4 MiB 不触页映射，再填充自有 8 MiB；不触碰 dyld/系统映射。
DATA 自施加纯 deny-default Seatbelt（不含第一批文件槽允许项）；MEMLIMIT 不施加它，以免把沙箱拒绝
混为系统权限拒绝。无 root、签名、注册、配置、真实媒体、网络和其他进程操作。

## 2. 判定

EPERM 只说明本机普通调用者的该入口拒绝，不能外推所有 Apple 扩展或系统代理。
DATA 的设置、新映射、既有映射分别记录；若设置失败，观察为不可用/未决，
而不是“内存已限制”。任何实验异常停止后续启动，保留失败产物。
捕获成功与机制成功分开；运行器只保证有限执行及原始记录，不是产品资格验证器。

公开 XNU 与当前运行内核不保证一致，源码推断和本机实测在结果中分别记录。
本批不能满足共享/压缩/GPU/内核记账、镜像 custody 或完整生命周期门禁。

## 3. 源码与公开合同核查

核查日期为 2026-09-20。Apple 公开 XNU 固定到
`f6217f891ac0bb64f3d375211650a4c1ff8ca1ea`（提交时间 2025-10-16），不是本机
macOS 27 / Darwin 运行镜像的源码证明。以下为源码事实及明确的应用推断：

| 入口 | 核查结果 | 对本项目的意义 |
|---|---|---|
| Mach footprint setter | `task_set_phys_footprint_limit` 调用权限检查，失败返回 KERN_NO_ACCESS | 与此前本机拒绝相符；本批不重复该调用 |
| memorystatus command 7 | 普通命令需 root 或 `com.apple.private.memorystatus`；免权限分支不含此命令 | 值得实测是否同样拒绝；普通开发者签名不等于获得私有权限 |
| Jetsam command 5/6 | 还受 CONFIG_JETSAM 条件限制；5 是 non-fatal，6 是 fatal | 不把 iOS/条件编译路径或告警当 macOS 可配置硬限额 |
| RLIMIT_DATA | `vm_map_set_data_limit` 和映射准入均使用 map size；低于当前 size 拒绝 | 与 AS 类似，不能凭名字当成独立物理内存限制 |
| RLIMIT_FOOTPRINT_INTERVAL | 仅重置区间峰值统计 | 不是限额 setter，不值得用压力实验“验证” |
| ledger syscall | `LEDGER_LIMIT` 分支受 LEDGER_DEBUG 编译条件控制，普通构建分支返回 EINVAL | 内核内部有 ledger_set_limit 不等于产品可调用的通用配额 API |

来源：[task.c](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/kern/task.c#L7582)、
[memorystatus 权限分派](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_memorystatus.c#L9153)、
[命令与 ABI](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/sys/kern_memorystatus.h#L327)、
[VM map 限额](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/vm/vm_map.c#L3677)、
[区间统计](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_resource.c#L3567)、
[ledger syscall](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/sys_generic.c#L2251)。

本机 SDK 的 `RLIMIT_RSS` 是 AS 别名，MEMLOCK 只限制锁页，不能代表总内存。
本机 `man launchd.plist` 说明 HardResourceLimits 调整 setrlimit；不因
ResidentSetSize 键名便推定另有硬 footprint 上限，未注册任何 launchd job。
`kern_memorystatus.h` 不在公开 SDK 中，但 libsystem_kernel.tbd 导出该符号；
研究声明采用上述固定 ABI，不把可链接视为受支持产品接口，也未复制 syscall 编号。

`phys_footprint` 本身是有明确组成的账本，不等于 worker 造成的全部系统成本。
XNU 单独记录内核内存、共享和带不同 footprint 属性的项目；未来即便获得一个
footprint 限额，也必须按产品资源合同逐项证明共享/COW、压缩、内核对象、GPU、
Broker/系统服务代办成本的覆盖或拒绝路径，不能把一次 setter 成功当总量证明。
参见 [task ledger 定义](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/kern/task.c#L1222)。

另核查 [Apple Enhanced Security helper 文档](https://developer.apple.com/documentation/xcode/creating-enhanced-security-helper-extensions)
和 [AppExtensionProcess](https://developer.apple.com/documentation/extensionfoundation/appextensionprocess)，
以及本机 ExtensionFoundation 的 macOS Swift interface：它们提供更严沙箱、
编译/运行时加固和系统管理的通信生命周期；本次阅读的公开接口中**没有找到宿主
可设置的每任务硬内存配额合同**。这是证据缺失，不是断言系统内部绝无扩展限额。
进程可复用、invalidate 表示连接失效，不能直接替代独占 worker 和终止回收。
签名身份缺失不是证明其永远不可行；获得签名也不会自动解决上述资源合同问题。

## 4. 本机实际结果

命令：`sh scripts/native-research/r0b-002.sh`。证据目录：
`/tmp/mengxia-r0b002.Nvr2e5ch`。UID/EUID 均为 501；arm64 macOS 27.0 / 26A428，
Xcode 27.0 / 27A266a，SDK 27.0；HEAD 为 `4fcf3470a6a98ad09feaf12152cee8c69740e467`，
带未提交修改。每个 case 三次均 exit 0、signal 0、confirmed reaped，stderr 空。

| 观察 | 三次结果 | 判定 |
|---|---|---|
| self memorystatus active/inactive 128 MiB fatal 请求 | rc=-1，errno=1 (EPERM) | H3 在该普通进程 tuple 下不成立；无提权重试 |
| DATA 64 MiB 请求 | rc=-1，errno=22 (EINVAL) | 该设置不可用；不当成限额生效 |
| DATA 当前 VAS + 1 MiB | 设置成功且软/硬限额回读相符 | 仅证明限额状态 |
| 此后申请新 4 MiB 映射 | errno=12 (ENOMEM) | 被测新映射拒绝 |
| 填充已有自有 8 MiB 映射 | VAS 不变；footprint 增加 8,388,608 / 8,405,016 / 8,388,608 字节 | H4 被反例否定；不是完整物理上限 |

六个原始输出经另外的只读字段检查逐项核对 UID、完成字段、errno、读回、VAS
和 footprint 差值；不是独立通用 schema 验证器，也不冒充 R0-A schema-2。
clang `-Wall -Wextra -Werror` 编译、clang 静态分析、Shell 语法、Rust fmt、
`git diff --check` 均通过；`./scripts/verify-repository.sh docs` 的 27 项测试通过，
并已将本报告纳入当前路线一致性及其负向断言。旧 controller 和 R0B001 两个
源码文件的 hash 与上一批记录一致，未重跑或改写它们的历史证据。
没有高负载、共享/GPU 压力、真实媒体、签名扩展、hosted CI、Formal 或第二 UID。
临时证据可能被系统清理；源码快照/编译参数/工具 hash/host 元信息由 runner 保留，
重跑产生独立目录，不覆盖旧结果。查询源码时 raw.githubusercontent.com 的 DNS
超时，改用 Apple 官方 GitHub 仓库 contents API 读取同一固定 commit；未运行下载代码。

SHA-256：

- r0b-002.c：`640e101a1d89fee8ca0fb2996b24dc65d1bb7deddb02df4a703c7a7abb2beae8`
- r0b-002.sh：`4df4bfd44a4b26298ec6e78e387ec5faba907942f3efd013fb8ae48ce45f7079`
- probe：`7404b83da5955e1decc5b425f0a1c9f7f06024498b04f9d4167e57a00ef31a51`
- controller.stdout：`7b93ea23ccc3a371df0ad6ebb76246f44be325d5286a7e75919e4c6142a7a056`
- sha256.txt：`994406ff352cbd9860113212b8edd44a8d6dd5718c168adbe65c9c50bbec5b9c`

## 5. 收敛与下一步

本批完成后有限实验授权结束，产品 authority 仍为 NONE。TASK-012 仍 BLOCKED；
不转向 TASK-015，不改变已完成任务、安全合同或产品功能。

明确停止把 **普通原生进程 + Seatbelt + AS/DATA headroom** 当作完整硬内存方案
继续扩建；两个映射限额反例和第二个权限拒绝已足以否定该组合的当前论证。
这不是所有 macOS 原生架构的全局 NO-GO；用户态封装不能凭自身创造缺失的内核权限。

下一阶段应是**执行边界的候选准入比较**，而不是再测同一 API 或直接写完整后端：

1. Apple 受管 helper：先取得可依赖的配额/记账/生命周期合同或新的明确机制；
   仅“能启动、能通信”不足以推进 R1。向 Apple 咨询或签名/注册实际扩展须另行授权，
   不把继续研究默认为允许对外提交信息或更改系统。
2. 若仍无该证据，提交保持 macOS 原生应用/业务核心、仅改变执行承载边界的方案
   比较（例如隔离执行环境），明确启动/体积/媒体兼容/硬件加速/离线/资源代办成本。
   这是待审的架构选择，不是已经采用 VM、Wasm、Linux 产品或特权 helper；
   任何实际引入均须用户接受及 ADR，资源总量也仍须独立验证。
3. 纯用户态 allocator、RSS 轮询 kill 或信任插件不能替代现有保证；若只选择这些，
   就属于安全目标变更，必须明确批准，不能包装成“补齐原生能力”。

启动新状态改变实验前必须列出能改变结论的新条件、精确预算和停止点。无新条件
则保留本结论，不以反复改文档或重跑失败探针制造进展。
