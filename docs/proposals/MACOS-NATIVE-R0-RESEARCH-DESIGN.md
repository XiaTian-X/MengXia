---
title: "macOS Native R0：有界可行性研究设计"
document_role: "Research design; not a product implementation or qualification gate"
status: "R0B_REOPENED_NO_PRODUCT_AUTHORITY"
version: "0.2.4"
date: "2026-09-20"
repository_head_reviewed: "4fcf3470a6a98ad09feaf12152cee8c69740e467"
---

# macOS Native R0 研究设计

## 1. 决策与边界

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


本文细化 [原生补全路线](MACOS-NATIVE-SUPPORT-DEVELOPMENT-PLAN.md) 的 R0，
不另设产品路线。目标是用有限实验回答“哪些 OS 机制足以支撑一个可实施的后端”，
不是先建设完整插件平台，再发现资源保证不成立。

最新排期：用户明确继续解决原生问题，[第一批](MACOS-NATIVE-R0B-001.md)与
[第二批](MACOS-NATIVE-R0B-002.md)有限实验均已完成，批次授权结束。
第二批确认另一限额入口拒绝及 DATA 既有映射反例；接下来按第二批 §5 比较执行
边界候选，不重复已失效的普通进程组合论证，不推断所有原生架构永远不可行。
该比较现已完成，见 [候选比较](MACOS-NATIVE-EXECUTION-BOUNDARY-COMPARISON.md)；
下一状态改变实验需先完成兼容性/承载选择及精确 gate，当前未授权 VM 或扩展启动。
这是有限研究重开，不是 TASK-012 产品启动，不把 TASK-015 草案当作当前动作。
以下为先前 R0 完成与纠正记录：R0-A 有界研究工具和实机观察已经建立；
R0-B 仅完成不改变系统状态的前置核查，因缺少签名身份、候选注册/生命周期实测和
完整资源边界而按本文规则汇合为 `INCONCLUSIVE`。研究 authority 随交付结束，
实施与产品 authority 均为 NONE。该结论不是 TASK-012 产品实施、DONE、R1 准入或
生产授权；后续只有新的平台条件和另行批准的精确候选实验才能改变它。

产品仍按 ADR-0020 内置优先；PLAN_FOUNDATION/BROKER_FOUNDATION 的纯合同工作
不等 R0 结束。Ubuntu、第三方安装/激活/执行保持延后且禁用。没有 root/helper、
Wasm/VM、放弃硬资源约束或 unconfined fallback 的隐含选择。

差异分类：R0-A 研究包的 EXPECTED_GAP 已由本次交付关闭；完整内存/磁盘边界、
扩展身份及生命周期仍为 UNKNOWN；旧 TASK-012 对 RLIMIT_AS 的否定依据为已记录的
SPEC_STALE。本设计不追加架构决定，不重新解释历史 DONE 或修改现有任务验收。

## 2. 分批交付与技术选择

| 批次 | 解决的问题 | 交付与边界 |
|---|---|---|
| R0-A | 普通权限下的机制观察、反例、可复现研究工具 | 自编固定 C 探针，只作用于自身及私有临时文件；无签名、系统注册、外部工具执行 |
| R0-B | 真实候选启动模型能否组成完整执行边界 | 最多比较 Apple helper extension 与最小 Seatbelt launcher；各自先闭合签名/文件/清理/试验保护条件 |
| R0 汇合 | 是否值得进入一个后端的 R1 | 有证据的选择或明确否定/未决，不以 probe PASS 代替资格 |

R0-A 可独立完成；R0-B 缺签名、设备或注册权限只阻塞对应实验，不判定整个原生
方向不可行。也不能只完成 A 就宣布内置执行或第三方 Native 可用。

优先顺序为资源覆盖 → 生命周期/身份 → 加载闭包 → IPC 适配 → 最小媒体功能。
每批结束集中更新一份结果，不随每个函数重写全局文档。除新反例、范围或公共合同
变化外，不重新打开已闭合问题；没有新事实不重复失败试验。

## 3. R0-A 文件、构建与接口设计

以下是已完成 R0-A 的精确研究实现范围：

| 文件 | 唯一职责 |
|---|---|
| `scripts/native-research/README.md` | 使用方式、预算、禁止事项、结果解释 |
| `scripts/native-research/run.sh` | 只读工具观察、固定构建命令、临时目录、调度与结果校验 |
| `scripts/native-research/controller.c` | 自有子进程身份、期限、有限输出采集、退出确认 |
| `scripts/native-research/probes.c` | 闭集 case 分发；无任意命令/路径/代码输入 |
| `scripts/native-research/cases.tsv` | 本文 case、固定参数、预期与适用范围 |
| `scripts/native-research/verify-evidence.awk` | 严格、有限输入的研究证据验证 |
| `scripts/native-research/self-test.sh` | 证据负例和有限 controller 故障夹具 |
| 本文及原生补全路线 | 研究结果与阶段状态，不更改 canonical 安全合同 |

不修改 Cargo workspace/lock、18 个 package、生产源代码、协议、迁移、稳定 AC/TEST
注册表、任务状态台账、既有架构测试或 CI。研究 case 名使用 `R0A_*`，不冒充
canonical TEST ID。若现有门禁确需额外兼容文件，先报告具体冲突，不静默扩范围。

使用系统 POSIX sh/awk、当前已观察的 Xcode clang 与 public SDK C 接口；不安装
库、不新建 Cargo package、不动态下载脚本。复用 `scripts/dev-toolchain.sh inspect`
的只读工具观察和信任检查，记录其 fingerprint；不执行 prepare/network、修改 pin
或把 developer observation 认作正式 attestation。R0-A 不依赖 ExtensionFoundation。

已实现接口为 `run.sh self-test`、`observe`、`run-a`、`verify <evidence-dir>`。
`run-a` 只运行新构建的固定 probe；`verify` 只读数据，
不能执行目录中的程序、source shell、跟随指向外部的证据链接或接受任意命令参数。
无“运行用户插件”“加载任意 sandbox profile”或接受外部 PID 的接口。

固定构建基线：clang 的绝对观察路径、`-std=c11 -O0 -g -Wall -Wextra -Werror`、
显式所选 SDK；controller/probe 各自独立二进制。禁止从环境继承额外 compiler/linker
flags 或 DYLD 注入；完整 argv、工具/SDK 标识及产物 hash 入证据。若 SDK 需要特定
声明开关，在固定源码中显式定义并审查，不自动移除诊断或追加私有 entitlement。

## 4. R0-A 保护模型与有限预算

**下列值是本研究包拟采用的保守实验预算，不是产品限额或 OS 强制保证。**
探针是可审查的有限程序，不是恶意插件；控制器不是通用安全沙箱。

| 对象 | 固定上界/行为 |
|---|---|
| 执行规模 | 12 个 case；每 case 最多 3 次，总计最多 36 次；串行，一次仅一个 probe |
| 进程 | probe 不 fork，不启动工具；仅指定 case 可 self-exec 一次；无后台服务 |
| 期限 | probe 自身 alarm 5 秒；controller 以单调时钟计算 10 秒期限，TERM 后 1 秒、KILL 后再观察 1 秒；整批调度上限 600 秒 |
| 构建 | 每个受信 compiler invocation 120 秒调度期限；独立于 probe 预算，无递归构建 |
| 显式内存请求 | 单次 malloc/mmap 最多 32 MiB，case 累计最多 64 MiB；触碰 payload 最多 8 MiB；拒绝整数溢出 |
| 映射枚举 | 仅自身元数据，最多 4096 项；达到上界则结果不完整，不改现有映射保护/内容 |
| 文件试验 | 每 case 最多 2 个文件、每文件最多尝试写 96 KiB，累计尝试写 192 KiB，缓冲区最多 32 KiB |
| FD 试验 | 最多创建 32 个额外自有 FD；每轮关闭；不设置按 UID 影响其他进程的全局限额 |
| 输出 | 每 case stdout/stderr 各 32 KiB；超限终止并判观察不完整，不能截断后继续判成功 |
| 留存 | 单批证据最多 4 MiB；最多 36 组有界输出，metadata/索引另限 512 KiB；不存 core dump |

这些上界覆盖显式试验操作，不声称限制 dyld/framework/compiler 全部内存、内核开销
或临时文件物理占用。特别是现有大 VAS 保留区，不纳入“显式申请 64 MiB”的保证。
没有已经合格的独立容错保护前，不运行无界分配/写入/fork、真实恶意样本、
GPU/IOSurface 压力、任意媒体解析、杀父留子或宿主全盘/全内存耗尽试验。

probe 只继承明确列出的 stdio、握手及私有目录所需 FD，其余关闭；环境采用有限
白名单，不透传 token、DYLD 或其他注入变量。controller 不改变调用者的限额。
controller 用持有的直接子进程关系管理唯一 PID；退出一经 reap 即失效，不使用
来自输出/磁盘记录的 PID，不向未知进程或共享进程组发信号，不使用 pkill/killall。
设置限制、建立输出通道和启动握手任一步失败都不继续该 case；probe 在实际测试
动作之前完成 core-disable、自身 alarm 和参数检查。exec case 必须重新安装 alarm，
保留 controller 期限；不得把 exec 前的运行时状态默认视为已保留。

期限是调度规则，不承诺内核卡住时一定按时完成 reap。退出未确认标记
`CLEANUP_UNCONFIRMED`，停止整批后续启动、不发布完整结果、不复用该实例；
保留活控制器持有的身份。控制器已经退出后不得靠保存 PID 续杀。Ctrl-C/TERM
先进入同一清理流程；controller 被 SIGKILL 等不可捕获故障的完整处理留给后端资格，
本批不声称已证明，也不在日常宿主故意制造该故障。

目录由 runner 在临时区新建，owner-only，固定内部文件名；拒绝复用现存目录、
symlink 或真实 Library。只写自身目录，不自动递归清理、不覆盖历史证据；结束
报告绝对目录与残留。注册、挂载、网络、Keychain、用户文档和系统设置均不访问。

实现位置为 ignored `target/native-research` 中新建的 owner-only evidence/build/work
目录；其既有父目录不是可覆盖的实验目标。self-test 单独使用 mktemp 所得私有根，
结束仅清理该根并确认不存在；它不清理 real evidence 或其他历史目录。

## 5. R0-A 实验矩阵

所有参数编译/清单固定，不接受自由数值扩大预算。正常预期之外的结果保留原始
证据，不自动调高限额重试。每个 case 3 次，用于发现不稳定，不宣称统计完备。

| Case | 动作与观测 | 可回答的问题/不能外推的结论 |
|---|---|---|
| `R0A_BASELINE` | 新进程报告 getrlimit、VM/task 自身统计与 PID；零 payload | 当前运行时基线，不是安全资格 |
| `R0A_ALLOC_CONTROL` | 未降低 AS 时依次 malloc/mmap 32 MiB，各自释放，不触页 | 对照路径能否成功；失败则相关拒绝实验不具有充分解释力 |
| `R0A_SMALL_CONTROL` | malloc 8 MiB，有限触页并释放 | 小型正常分配控制；不是整进程 footprint 上界 |
| `R0A_AS_SMALL` | 读取当前 VAS，加 16 MiB 作 soft/hard，申请 8 MiB并有限触页 | 限额内是否正常；读数/allocator 变化导致失败亦必须记录 |
| `R0A_AS_MALLOC` | 同样降低 AS，malloc 32 MiB，不触页，成功则释放 | 指定分配路径是否被拒绝 |
| `R0A_AS_MMAP` | 同样降低 AS，匿名 mmap 32 MiB，不触页，成功则 unmap | 指定映射路径是否被拒绝 |
| `R0A_AS_EXEC` | 降低 AS 后一次自身 exec；核对继承、尝试提高 hard、再申请 32 MiB | 同镜像继承与 hard 提升拒绝；不代表不同镜像/预加载/子树 |
| `R0A_AS_FIXED` | 尝试固定 2 GiB AS 并记录结果，不分配 payload | 重现固定值失败/成功及 errno；失败不否定相对 AS 所有作用 |
| `R0A_VM_REGIONS` | 有界读取自身区域大小和保护位 | 提供 VAS 分解；不是 RSS/footprint，也不能减掉某类区域宣称硬上限 |
| `R0A_FSIZE` | 降低自身 FSIZE 为 64 KiB，两文件各尝试有限写入；捕获 SIGXFSZ/errno，控制器 fstat | 验证单文件与总量不同：两个文件可合计超过 64 KiB；不算总磁盘限额通过 |
| `R0A_NOFILE` | 新进程确认已用 FD 不超过 16 且继承 hard 至少 32，将自身 soft/hard 降为 32，尝试最多 32 次 dup | 应看到正常成功再到 EMFILE；前置不满足或未到拒绝点均未决，不无限追加；不证明 exec 继承 |
| `R0A_DEADLINE` | 固定有限等待，probe alarm 5 秒；controller 测试专用期限 1 秒并清理 | 验证 controller 主动清理及退出确认；不使用无限循环夹具 |

降低 AS 采用 checked addition，目标不得高于继承 hard 上限；条件不满足记
`INCONCLUSIVE`，不修改父进程限额。硬限额只在隔离的一次性 probe 下降低。
保留每一步返回值、即时 errno、初始/最终读数、成功分配大小、已触页数量及
controller wait 状态；不得只打印一个 PASS 或以异常退出代替“OS 拒绝了分配”。

`R0A_AS_EXEC` 降限后 exec 失败是有效观察，但继承/拒绝子项未测，不能通过该项。
控制 case 的正常成功与实验 case 的明确拒绝须配对，否则结论保持未决。
Mach footprint 限额返回 8 的已有证据作为历史输入引用，不重复无新条件的尝试。

## 6. R0-B 候选闭合清单

本节冻结问题与出口，不伪造尚未选定的签名身份、bundle ID 或系统注册命令。
实施前只补齐所选候选的精确工程文件、宿主/签名条件、有限夹具与撤销清理步骤；
需要更改系统状态时取得明确授权。条件不足的项记 NOT_RUN，不静默用 mock 代替。

| 维度 | Apple helper extension 候选 | 最小 Seatbelt launcher 候选 |
|---|---|---|
| 启动与部署 | 检查 app/appext 包装、CLI/daemon 无 UI 入口、离线发现、签名/注册后启动 | 验证允许使用的系统接口、sandbox 在复杂初始化/加载前生效；记录非稳定 API 维护成本 |
| 生命周期 | 实测进程复用、重连、invalidate、宿主退出及实例终止确认 | 验证 owned process、禁止后代/受控后代、重复 exec、取消与父死后的收敛 |
| 身份与 custody | 包摘要到已签名/注册/实际运行镜像的绑定，更新及注册竞争 | 启动前镜像固定、加载闭包、路径/FD/环境替换与检查使用竞争 |
| 内存 | 明确是谁能在首次不可信代码前设置不可提高的限额，覆盖哪些记账路径 | 同样要求；相对 VAS、自定义 allocator、轮询峰值都不自动等于总内存硬限 |
| 可写空间 | 明确全部可写渠道和总量边界，包括宿主代写/临时输出 | 同样要求；单文件 FSIZE、剩余磁盘检查不等于总量限额 |
| 通信 | XPC 身份验证后如何桥接现有字节协议，复制/缓冲/取消/EOF的预算 | 有限 pipe/socket 与继承句柄；现有握手不是 OS 进程身份证明 |
| 功能 | 在已建立的有限保护下使用自有小夹具验证必要媒体能力 | 同左；不得未经保护运行任意下载的 FFmpeg/插件/媒体 |

Apple 官方明确支持复用运行中的扩展，且断开连接不自动终止进程。因此不能直接
把 AppExtensionProcess/XPC connection 当作一次 Run 的独占 owned child。
若候选无法证明旧 Run 已终止并释放权限/预算，保持不合格，不能靠重新握手清零。
不以文档未列出资源 knob 就断言平台永远无解，也不以 Enhanced Security 名称
推断已有内存/磁盘总量保证。[Apple 进程模型](https://developer.apple.com/documentation/extensionfoundation/appextensionprocess)、
[扩展宿主生命周期](https://developer.apple.com/documentation/extensionfoundation/adding-support-for-app-extensions-to-your-app)、
[Enhanced Security helper](https://developer.apple.com/documentation/xcode/creating-enhanced-security-helper-extensions)。

资源研究优先检验“可阻断的渠道 + 普通权限实际强制机制”的组合，不自行虚构内核
能力。纯 Broker 输出预算仅能覆盖其独占代办的渠道；guest 能直接写的临时区、
文件映射或共享服务必须另有边界或实际禁止。内存须明确匿名、既有保留区、COW、
文件/共享映射、内核代办及可达 GPU/IOSurface 渠道的覆盖/拒绝方式。
可缩小支持 profile，但必须保留其真实功能可行性并经 R1 明确接受，不能删除失败
case 后声称支持“任意 Native”。新的磁盘映像/配额机制试验需独立有界范围，
R0-A 不挂载、不创建卷、不调用特权或未经支持的接口。

## 7. 证据设计：完成采集不等于安全通过

每批保存固定结构：`metadata.tsv`、`cases.tsv`、`summary.tsv` 以及
`case-<closed-id>-<1..3>.stdout` / `.stderr`。二进制在独立 build 子目录，
不计入证据 4 MiB 上限，但逐个记录 SHA-256；不归档系统工具/SDK。
纠正后的 metadata 记录 schema=2、budget_version=R0A-2、时刻、OS/build/arch/SDK/Xcode、工具观察 fingerprint、
HEAD、dirty 标志、本包每个输入文件的 hash、精确构建 argv/产物 hash、预算版本。
dirty 不能只记 HEAD；不收集整个环境或无关 diff，避免带入秘密。

TSV 是数据不是脚本；封闭键/enum/相对文件名、数值长度和行数有上限，文本需
明确转义 TAB/LF，拒绝未知字段、重复主键、绝对/上级路径、symlink、超限内容与
缺失文件。verify 先检查体积再解析；不从被验证数据读取执行命令或期望清单。
case/attempt 集合须与受审的 cases.tsv 精确一致，结果文件在写完并确认退出后
封闭；hash 证明内容对应，不是签名、可信时间或不可伪造的安全 attestation。

case 结果为 `OBSERVED_EXPECTED`、`OBSERVED_COUNTEREXAMPLE`、`INCONCLUSIVE`、
`NOT_RUN`；另记 exit/signal、cleanup 状态和子断言。反例可构成**完整且有价值**的
研究结果，不能为了绿灯改成 expected。self-test 的合成材料放独立临时目录，
带 SYNTHETIC 标记，永不计为实机观察。probe 输出与 controller 的 wait/fstat
交叉核对；文字“PASS”本身不是证据。

命令结果分开：退出 0 表示证据结构完整且全部指定观察已完成（可以含反例），
退出 2 表示未决/缺测/环境不满足，退出 1 表示工具/格式/清理错误；stdout 始终
给出研究结论，绝不输出产品 ENFORCED/PASS。verify 不替代人工对安全覆盖的判断。
静态资料核查与历史证据单独标记来源，不混作本批实测。

## 8. 验收与停止：避免无止境研究

先实现 runner/controller/evidence 的负向 self-test，再执行 A 的固定 case。
负例至少覆盖缺 case/重复 attempt、伪造来源 hash、截断/超限、路径逃逸/symlink、
合成材料混入、非零/信号退出冒充分配拒绝、exec 失败冒充继承、清理未确认。
故障模拟验证结果解析和调度分支，不声称证明真实内核故障行为。

R0-A 接受条件是源码和预算被复核、self-test 通过、每项有真实观察或明确未决原因、
保存有限原始证据及反例。它可以以“资源问题仍未解决”完成研究交付，不能因此
消费为 R1 可行性或 TASK-012 completion。三次不稳定结果保留为未决，不择优。

R0 汇合只作以下三个决定之一：

- `GO_FOR_R1_REVIEW`：至少一个候选对必要资源、启动/身份、生命周期有实际机制
  与限定范围的正反证据，关键绕过有可验证防线，功能没有被禁用到不可用；列明
  尚需 R2 完整故障/hostile 验收的义务。只是进入后端 ADR/启动评审，不准入产品。
- `NO_GO_FOR_CURRENT_CONTRACT`：已证明候选无法满足当前所需合同，记录最小反例
  与影响范围；不把候选失败外推为所有 macOS 原生技术不可能。
- `INCONCLUSIVE`：缺平台条件、关键路径未测或结果不稳定，准确列出缺项与下一项
  能改变结论的试验；无新条件不重复同样实验，不准入依赖该结论的执行能力。

两候选都不能满足时，停止本轮后端扩张，交由用户选择新的范围/架构/风险决策；
纯基础开发继续按自己的 gate 推进。不接受监控杀进程冒充硬限额、无保护内置
临时版或无限增加候选。正式资格仍属于 R2，真实工作流仍属于 R3，第三方开放仍
属于 R5；现有 TASK-010/TASK-011 只在后续被证明需要演进的合同处精确修改。

## 9. R0 完成记录与汇合结论

### 9.1 R0-A：初版历史观察（工具验收已由 §9.4 纠正）

七个研究文件均已按 §3 建立，没有修改 Cargo workspace、生产源码、协议、迁移、
稳定 AC/TEST 注册表或 CI。`self-test` 的 11 个负例全部被拒绝；最终 `run-a` 在
arm64 macOS 27.0 build 26A428、Xcode 27.0 build 27A266a、SDK 27.0 上完成 12 个
case × 3 次串行观察。结构校验结果为 36 个 `OBSERVED_EXPECTED`、0 个反例、
0 个未决、0 个未运行；每次均确认 child 已退出并 reap，证据总量 13,803 字节。

最终开发证据位于被 `.gitignore` 排除的
`target/native-research/evidence.doKYy0hM`。它不是提交物或正式 attestation；其
`metadata.tsv`、`cases.tsv`、`summary.tsv` SHA-256 分别为
`bd82c03c0a782ba32de4ebb114b30e458e3aded986eee0d05622a3e71a70748a`、
`90679bb7316b74141d19f88392c12932b128cfa86ae2f1d214a7097111fe2536`、
`88f4d475abf27b689a8420adf54cbb8fcb2d2ca09c131a1b1fe09fb9f5f83315`。
metadata 明确记录 HEAD `4fcf3470a6a98ad09feaf12152cee8c69740e467` 且 dirty=YES，
因此不得把目录或 digest 描述为干净提交的不可伪造证据。

观察在三次运行中语义一致：正常 32 MiB malloc/mmap 与触页 8 MiB 控制成功；以
当前 VAS + 16 MiB 设置相对 `RLIMIT_AS` 后，触页 8 MiB 成功而 32 MiB malloc/mmap
以 `ENOMEM` 被拒；同镜像 exec 保留 soft/hard limit、提高 hard limit 以 `EPERM`
失败。固定 2 GiB AS 以 `EINVAL` 失败，说明不能把固定绝对值当可部署配置。
`RLIMIT_FSIZE=64 KiB` 仍允许两个文件合计 128 KiB，证明它不是目录总配额；
`RLIMIT_NOFILE=32` 在 22 个额外 dup 后到达 `EMFILE`；1 秒 deadline 路径以
SIGTERM 收敛且 reap 确认。VM inventory 完整但约 500.5 GB 的虚拟区间不是 RSS、
物理 footprint 或共享/GPU 记账证明。

### 9.2 R0-B：条件不足，未执行会改变系统状态的候选实验

只读前置核查确认 SDK 27.0 含 ExtensionFoundation `EnhancedSecurity`、
`AppExtensionProcess` 和 `invalidate()` 接口，但本机 code-signing identity 为 0。
没有获准或尝试 app/appex 签名、注册、启动、复用、终止或 XPC 身份实验，因此
Apple candidate 的部署、实际镜像绑定、独占 Run 与生命周期均保持 UNKNOWN。

系统 `/usr/bin/sandbox-exec` 当前为 root:wheel/0755、Apple 签名，identifier 为
`com.apple.sandbox-exec`，SHA-256 为
`58839ef01b4eef8aac0d2aa8f9d1c074ae45aafe3533965b030672450064acc8`，CDHash 为
`1266e34f192210d9f1ac6dbc9cf297ecff93e568`。它与历史 macOS 26 记录不同，故旧
tuple 证据不能迁移到当前系统。未运行 profile、hostile fixture、媒体程序或任意
外部 executable，也未改变签名、注册、系统设置或 mount 状态。

### 9.3 汇合

`R0_CONVERGENCE: INCONCLUSIVE`。R0-A 完成了其有限研究义务，也确认相对地址空间、
单文件、FD 和 controller deadline 的具体行为；它没有闭合物理/共享/COW/GPU
内存、聚合可写空间、镜像 custody、候选生命周期、IPC 身份或真实媒体功能。
R0-B 的关键路径未测，现有证据不足以选择任何后端进入 R1，也不足以宣称当前合同
不可实现。TASK-012 继续暂停并保持 `DRAFT / BLOCKED / authority NONE`。

能改变结论的最小新条件是：为**一个**候选建立精确、另行批准的实验包，并具备
受控签名/注册测试身份或受审 Seatbelt tuple；随后在不可信代码首次执行前证明
镜像绑定、完整资源覆盖、独占生命周期和可确认终止。没有这些条件不重复 R0-A。
该轮的 PLAN_FOUNDATION 排期已被新的 R0-B 优先级取代，仍不能消费 R0 为执行资格。

### 9.4 审查修复与重新验证 — 2026-09-20

用户明确授权修复 R0 工具，不启动 TASK-012/R0-B。分类：实现与设计不一致为
REPO_STALE；初版工具完整验收表述为 SPEC_STALE。§9.1 保留为历史，不能证明
纠正后的清理、FD、语义和来源保证。本节取代其当前工具验收，不抹去原始观察。

集中完成的修复：

1. 取消/overflow 只停止正常等待，清理继续 TERM→必要时 KILL→waitpid；未确认退出
   时保留 controller 对直接子进程的所有权，停止后续调度，不复用磁盘 PID。
2. 使用当前 SDK 的 public posix_spawn chdir 与 CLOEXEC_DEFAULT，显式接入
   stdin=/dev/null、stdout/stderr；不存在 4096 截断继承。仅为研究启动，不是沙箱。
3. schema 2 / R0A-2 严格核对原始字段、probe result、summary、预期 errno 和成功
   对照；FSIZE 增加 controller 自己的 fstat 观测。稳定反例可形成完整研究证据，
   观察不稳定则退出 2，不择优。schema 1 保持历史，不自动迁移为新证据。
4. 修正 malloc/mmap 及 exec hard-limit 的错误归因；非预期 errno 不算限额生效。
5. 修正三个 TSV 负例并逐项核对具体拒绝原因，增加语义矛盾、错误 errno、独立
   文件大小、缺字段、旧来源和缺对照等回归；合成材料始终独立标记。
6. 编译前冻结七个输入的只读快照，实际从快照编译并记录对应摘要；完成前检查
   live-source 漂移，避免把新源码归属于旧二进制。README 也纳入来源摘要。
7. 编译后的整批调度/证据收尾使用同一 600 秒单调时钟 deadline，controller
   使用剩余期限；到期拒绝新 case 或清理在途 case。runner 捕获取消并等待自有
   controller。不可中断内核/文件系统阻塞仍不作硬实时保证。
8. self-test 目录归属改为父 shell 持有的单一 mktemp 根，清理后确认不存在。

新增 controller 故障回归只编译于 `R0_SELF_TEST`：每个夹具有限等待/自身 alarm，
输出压力夹具最多写 64 KiB、只保留 32 KiB；无任意代码、无限压力或外部进程。
它们不计入 12×3 的实机机制观察，也不是产品 hostile conformance。

本轮最终验证：

- `scripts/native-research/run.sh self-test`：17 个按原因断言的负例、2 个证据结果
  测试、8 个 controller 场景、3 个 errno 分类断言通过；私有测试根清理确认。
  场景包含真实取消、有限输出超限、TERM→KILL、FD 5000 跨 exec 不继承，以及
  启动前/执行中的 batch deadline。证据测试确认稳定反例允许留存、不稳定退出 2。
- `scripts/native-research/run.sh run-a`：36/36 `OBSERVED_EXPECTED`；0 反例、
  0 未决、0 不稳定 case，所有直接子进程确认回收。FSIZE 的两份 65536 字节文件
  得到 controller 独立确认。没有重新运行签名/注册、Seatbelt 或媒体候选试验。
- 两个 C 文件的 clang 静态分析无诊断；文档门禁 27 项通过，Shell 语法与空白检查通过。
  未运行整个 workspace/Formal、第二 UID 或 hosted CI；生产代码未修改。

最终证据目录：`target/native-research/evidence.adK9JHgN`，共 14,108 字节；同一
macOS 27 / Xcode 27 / SDK 27 tuple，HEAD 不变、dirty=YES。摘要为：

| 文件 | SHA-256 |
|---|---|
| metadata.tsv | `8946c6658a913408aba418810af2880ed166a6de678bdccd562b9d5ec0171cf6` |
| cases.tsv | `90679bb7316b74141d19f88392c12932b128cfa86ae2f1d214a7097111fe2536` |
| summary.tsv | `d065060cdb071852de3867729f0c1ac4c8d98db040d210dff0cdc11ab0819089` |

这仍是未提交的 developer evidence，不是签名 attestation。构建快照在
`target/native-research/build.YZkpZaSC/source`，工作文件在
`target/native-research/work.VFHDlrQu`；旧证据未覆盖。修复范围是既有研究工具及
记录/版本台账，不改协议、迁移、生产依赖、规范强约束或历史产品任务状态。
本次研究修复已结束，临时修复 authority 撤销；产品 authority 始终 NONE。
汇合仍为 INCONCLUSIVE：R0 工具正确性修复没有解决后端总资源与生命周期可行性。
