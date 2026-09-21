---
title: "macOS 原生执行边界：候选准入比较与下一阶段设计"
version: "0.1.1"
status: "HISTORICAL_STRICT_COMPARISON_REVIEWED_NATIVE_SELECTED"
date: "2026-09-20"
---

# 原生执行边界候选比较

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

BOUNDARY_BACKEND_SELECTION: NONE
BOUNDARY_PRODUCT_AUTHORITY: NONE
BOUNDARY_NEXT_EXPERIMENT: NOT_AUTHORIZED

## 1. 结论与本次交付

本轮完成 R0B002 §5 要求的源码/公开接口/现有实现对照，没有运行新的探针。
**目前没有一个候选获得完整资格；不能承诺“换一种框架就解决全部问题”。**
已经足够明确的工程结论是：不再扩建被反例否定的 Seatbelt + AS/DATA headroom
组合；原生应用和不可信执行的承载方式可以分开设计，但不能掩盖兼容性变化。

本项目 V1 是 Rust CLI + Core/daemon，不是桌面 UI 工程。所谓保留 macOS 原生
主程序，是保留这些既有模块、macOS 文件系统/IPC/安全适配与业务数据，不引入
跨平台 UI 框架。现阶段平台 sandbox crate 仍是占位，真正后端尚未形成，不需要
推翻已完成的业务与存储代码。但隔离适配不是零成本，下面明确列出未来改动。

在用户偏好原生的前提下，推荐按兼容性做一次选择，只验证一条路线：

- **必须保留 Darwin/Mach-O 插件与 macOS API：** 优先评估 macOS guest 的隔离
  worker，受管 helper 保留为等待明确资源合同的备选。macOS VM 是 ABI 较接近的
  候选，不是已经可用或低成本的承诺；若不接受 VM，当前还没有满足原条件的路径。
- **原生主程序即可，worker 允许 Linux ELF：** 轻量 Linux guest 更值得作为下一
  个最小实验候选；可参考 Apple Containerization。代价是 guest 工具链/内核维护、
  包目标演进及媒体能力差异，不是“现有包原样运行”。这需用户明确接受。

两者都不自动通过硬资源门禁，也不启动 Ubuntu 产品。第一条符合更严格的原生
兼容解释；第二条通常有更小的 guest 系统范围，这是架构推断而非本项目实测。
不将未验证的性能/体积改善写成承诺，也不以第三方延后为由删除未来兼容性要求。

用户追问“如果不需要考虑插件兼容问题”：在这个**假设**下，推荐优先验证
macOS 原生 host + 最小 Linux VM worker，不优先建设完整 macOS VM。前者的
可裁剪系统、可复现镜像和 guest cgroup 更贴合有界执行需求；后者主要的 Darwin
API/包兼容优势不再决定选择，而完整 guest 系统的部署/维护成本仍在。
这不是用户已接受 Linux/VM 的决策。媒体加速需求也不能从“不需要插件兼容”
推导为可删除；若 Apple 本地硬件能力仍是首要要求，要先证明候选媒体路径。
Wasm 可适合未来纯计算/编排扩展，但不自动解决原生媒体工具和宿主总资源问题，
不建议为首个候选同时引入 Wasm 与两类 VM 三套运行时。

## 2. 候选矩阵

| 候选 | 实际提供什么 | 缺口 / 功能代价 | 本轮准入结论 |
|---|---|---|---|
| 普通进程 + Seatbelt + rlimit | 被测访问拒绝、新映射和单文件限制 | 既有映射增长反例；无完整硬内存边界 | 当前组合停止扩建；不是全部原生永久不可行 |
| Enhanced Security helper | 系统更严沙箱、编译/运行时加固、扩展管理 | 未找到宿主配置硬内存配额的公开合同；包、CLI 部署、进程复用和运行身份需适配 | 等新机制/官方合同，不先造完整桥接层 |
| Virtualization.framework + macOS guest | 同族 OS/ABI，guest 内存配置，整 VM 强制停止 API | 完整 guest 安装/更新/磁盘成本；硬件加速与 API 可用性另验；host 开销未闭合 | Darwin 兼容优先时的候选，尚不准生产执行 |
| Virtualization.framework + 最小 Linux guest | 固定 guest RAM、隔离 OS；可再用 guest cgroup | Mach-O 不兼容；Apple SDK/VideoToolbox 不能直接沿用；guest 供应链 | 允许 ELF 后才作为轻量实验首选 |
| Wasmtime / WASI | 模块内存、显式 imports、fuel/epoch 等执行控制 | 不是 Mach-O runtime；limiter 不覆盖全部 runtime/host 分配；原生 FFmpeg 执行问题仍在 | 可另立未来 Wasm 插件类别，不替代本次原生问题 |
| root/helper、私有 entitlement 或内核改造 | 可能接触普通用户不可用机制 | 改变安装、信任和运维模型；得到权限也不等于限制语义合格 | 本轮不采用、不实现、不提权 |

Containerization 是 Linux VM 的管理库/参考实现，不是第七种独立内核边界。
不能把 Docker 风格 CLI、容器名字或 VM 外壳直接记为 SandboxEvidence 全部 ENFORCED。

## 3. 可核验事实与不能外推的地方

### 3.1 受管 helper 的优势与出口条件

Apple 文档说明 Enhanced Security helper 运行在更严沙箱中并使用加固；没有在
本次检查的公开页面及 SDK 接口中找到宿主可设置的每任务硬内存预算。[S1]
这是支持合同证据缺失，不是证明系统内部不存在任何内存管理。

生命周期需要精确表述：`AppExtensionProcess` 可能连接已运行的进程；
`invalidate()` 的文档说明，**最后一个连接失效时系统会终止扩展进程**。[S2]
因此它不是“永远不会终止”，也不是每次调用立即证明独占 worker 已退出。
还需验证连接所有权、复用边界、退出通知/回收和 deadline 后的状态收敛。
没有内存合同前，签名注册后只做 Ping 不能改变当前可行性结论。

### 3.2 VM 内存配置确实改变边界，但不是总宿主配额

Apple 的 `memorySize` 指 guest 看到的物理内存；宿主预留虚拟区域并按需分配，
不等于启动时占满对应物理 RAM。[S3] 这使 guest 内预留映射的后续触页仍处于
guest RAM 边界内，与先前给巨大宿主 VAS 加 headroom 的论证不同。
这是新的、值得验证的候选机制，但仅据此仍不能证明整个宿主成本有硬上限。

至少分账：guest RAM；VMM/设备模拟/队列；guest 驱动的宿主系统服务；应用 Broker
缓冲与解析；并发 worker；临时磁盘/日志。不得遗漏其中一项，也不得把这些独立
分项相加时重复计算同一映射。`CPUCount` 只是虚拟 CPU 数，不是累计 CPU 时间
预算；guest cgroup CPU 配额也不能替代宿主墙钟期限和整体并发预算。

静态上限或有界队列推导、OS 强制语义与故障测试应共同支持结论；**仅测得峰值
较低不等于 host 开销已被强制限制**。若 Apple 黑盒路径没有足够的控制/记账
证据，仍不能宣布现有 resources 要求满足，必须报告剩余风险/合同决策。

`requestStop` 是请求 guest 自行关闭；`stop` 不给 guest 清理机会并可能返回
状态竞争/内部错误。需结合 VM 状态和 delegate 事件确认停止，不能把成功发送
请求或 XPC/vsock 断开当成停止证明。[S4]

### 3.3 guest 资源机制同样有语义边界

Linux cgroup v2 的 memory.max 提供内核内存限制，但官方允许某些情况下暂时
超限；memory.high 不是同等硬限额，swap 需独立处理。树终止有 cgroup.kill，
仍须观测退出而非只验证写入成功。[S5] 不能许诺“任何时刻零字节超调”。
后续 gate 必须写明记账对象、粒度、超限结果、配置继承/不可修改性、全局预留与
失效动作；这是接受机制语义，不是自动降低当前安全要求或提前验收 Ubuntu。

guest RAM 有限、禁用 swap/balloon 等可缩小变量，但不单独限制宿主虚拟磁盘、
VMM I/O 队列、host 内存压缩/swap 或服务代办成本。不得让 guest 获得配置控制权。

### 3.4 优秀开源实现可复用到什么程度

固定核查 Apple Containerization commit
`d29e00a7c17cb0f454c8945bd3a6bc447e233cfb`（2026-09-18），不是选定生产版本。[S6]
它提供每容器 VM、guest init 与 vsock gRPC 管理；无需自己开发 hypervisor。
与 MengXia 的 typed Broker、包 custody、审计和资源证据并不是同一责任。

代码核查发现：VZ manager 会裁剪请求 CPU/内存，instance 还会按 MiB 向上取整；
MengXia 若采用，必须先接受规范化后的有效配置和预算，不能用输入参数当证据。
该 instance 配置包含 virtio-fs 设备处理；严格“不暴露 host 目录/额外设备”的
候选需要适配或另选薄层实现，不能直接采用默认配置。[S7]
guest cgroup manager 的 applyResources 分支写 memory.max/cpu.max/pids.max，
不能由此推定所有资源和 swap 都已覆盖。[S8]

依赖图含 Swift/NIO/gRPC/归档/压缩/OCI 等组件；不是只增加一个零成本接口。
README 的运行要求与 Package.swift 的平台声明不能混为一谈，未来以确切版本、
供应链和本机 tuple 验收。库自己的启动性能声明不是本项目基准。[S6][S9]
本轮只读取官方源码，未拉取可执行镜像、未安装 container CLI、未执行构建。

Wasmtime 的 ResourceLimiter 明确不覆盖全部 Store/runtime 和宿主分配，当前
页面还说明 shared memory 有额外限制。[S10] Wasm 受控 imports 的模型有价值，
但 native host callback/JIT/编译/缓存/host 媒体任务仍须独立预算，不能搬入 Core
进程便声称整个系统安全。只有另立 Wasm 包和能力范围后才讨论实现。

## 4. 本项目的兼容性与已有实现

| 现有部分 | 事实 / 影响 | 后续处理，非本轮改动 |
|---|---|---|
| TASK-010 package | `manifest.rs:828` 仅接受 aarch64-apple-darwin | Linux guest 是真实合同变更；不得临时放宽字符串白名单 |
| TASK-010 历史字节/摘要/fixture | 既有 Darwin 包和证据仍有效 | 新 target/schema 通过独立 gate 演进；保留旧 canonical bytes 与拒绝行为，不重写历史 fixture |
| TASK-011 host/proto | caller-supplied AsyncRead/AsyncWrite；无 spawn；有 TerminationRequired | 有望复用帧/会话逻辑；vsock/XPC 桥须验证有界接收、反压、取消、stderr；请求终止仍交给新 supervisor |
| 平台 sandbox | 当前 `lib.rs` 是占位 | 新适配放在批准的平台边界；不得把 Swift/VM/unsafe 引入纯 domain/store |
| 存储/Client IPC/已完成任务 | host 上的 macOS 实现和证据 | 不把 Library DB/CAS/Client/Admin 端点传进 guest；不改历史 DONE，新增端到端回归 |
| FFmpeg | 后续功能未实现；规范要求受控可复现转换 | 每平台构建/codec/filter/字体/色彩/时间戳/缓存资格分别验证；摘要或执行环境改变不得误复用缓存 |
| Broker/Provider/Run | 产品实现仍未授权 | secret/network 留 host；guest 请求不等于授权；保留真实 Run/lease/审计与迁移顺序 |

Linux guest 不能直接使用 Darwin Mach-O 或 Apple VideoToolbox API；同 CPU 架构
不代表同 OS ABI。FFmpeg 官方将 VideoToolbox 描述为 Apple 平台硬件解码路径。[S11]
macOS guest 虽保留 OS 家族，也不能据虚拟显示设备就承诺 Metal/VideoToolbox 的
硬件能力和性能。首个 CPU-only 探针只是研究范围，不是永久删掉 GPU 功能。
若替代方案不能达到用户需要的媒体能力，要在选择阶段披露，而非实施后隐藏降级。

内置最小 Linux worker 不等于 Ubuntu 版交付，但确实新增 Linux guest 软件的
构建、维护和测试工作；若用户暂不接受任何 Linux 工程工作，这条候选也应搁置。
macOS guest 则增加恢复镜像、安装/升级、系统状态、发行条件核验；本轮未下载
镜像、未解释或确认任何发行许可，不能给出未经验证的固定体积/启动秒数。

## 5. 若选 VM，最小安全设计应怎样落地

以下是待接受的设计约束，不是已实现的保护或启动授权：

1. 第一版资格夹具每个执行实例独占一个 VM，先串行；不同包/租约不能仅因同一
   Run 就共享可见资源。取消整个 VM 不波及其他执行；不重用处理过用户数据的
   guest/snapshot。将来池化需独立证明清空和身份隔离。
2. macOS host 保有 Library、凭据、审核镜像、Run 状态与预算。guest 仅看自己
   的只读程序/输入和有限 scratch。最小配置禁用网络、目录共享、GPU/USB、
   clipboard、Rosetta、嵌套虚拟化及非必要设备；不通过通用 shell/URL 代办。
3. 精确核验 boot/kernel/rootfs/helper/包及配置，先建立硬边界，再加载不可信
   程序。guest 的 PID/UID/自报 digest 不作为 host OS 身份；host 创建的 VM
   实例、专用连接、一次性 challenge、预期包和执行实例共同绑定。guest agent
   是受信启动链的一部分；不得信任任意 guest 来电或仅凭 vsock port 授权。
4. vsock 可不经过 IP 通信，[S12] 但“没有网卡”不代表 host Broker 无法被滥用。
   host 在读取/分配前限定帧、连接、队列、总字节和并发，语义验证后再做授权。
   Swift/NIO/gRPC 如存在，也必须证明其内部 buffering 不绕过 TASK-011 上限。
5. 不共享真实 host 目录，不在 host mount guest 可写文件系统来收集结果。采用
   预先登记输出槽及有界字节流，host 自己创建/校验 staging 对象。guest 输出
   路径、符号链接、设备节点、文件系统元数据都不被当成可信 host 指令。
6. guest 虚拟磁盘容量不等于 APFS 总物理占用；定义镜像数量、逻辑/物理预留、
   COW/快照、宿主元数据、日志和输出总预算。核验真实存储边界，再测试媒体。
7. 单一 supervisor 持有创建/取消/终止/回收责任；断连、超时、daemon 重启、
   VM stop 竞争与错误都有闭合路径。停止未确认时不释放预算、不复用身份，
   不重试可能已产生的外部效果；guest 不接触 Provider 凭据和直连网络。

## 6. 后续阶段与停止点

不把研究阶段标签当作新的稳定 TASK/AC/TEST，不替换规范中的任务顺序。

| 阶段 | 做什么 | 进入下一阶段的必要结果 |
|---|---|---|
| 兼容性选择（当前出口） | 明确必须保留 Darwin worker，还是可接受 ELF worker；明确是否接受内部 VM | 用户选一条；不默认双后端，不把“开始下一步”当作采纳新架构 |
| 单候选有界研究 gate | 精确 SDK/运行镜像来源、签名/entitlement、文件范围、磁盘/内存/CPU/时限/输出预算和清理责任 | 审查并授权；没有候选镜像/硬件最低要求，不能编造精确预算或直接下载 |
| 无产品准入的机制验证 | 先合法配置/可信小夹具启停、通道绑定；再有限分配和输出边界反例 | guest 及 host 每项资源证据；停止/回收确认；UNKNOWN 不可变 PASS |
| 单后端 R1 选择 | 审查支持性、功能/性能样本、供应链、部署及未决风险 | 接受 ADR、精确平台 tuple、功能范围与后续 gate；或明确 NO-GO/INCONCLUSIVE |
| 限定实现与集成 | 按既有 R2/R3 和 TASK-012/013/014/015 作用域推进 | 非准入夹具后再完整安全矩阵，最后真实 Run/Broker 集成；不跳数据库/审计门禁 |

最先否决候选的检查应是 **host/VMM 总资源路径能否闭合**，其次是所需媒体能力、
包身份和生命周期；不要先造漂亮 API 或大规模插件 SDK。实测预算仅用于可信
小夹具，不将 host 上“几秒后 kill”当作 hostile 耗尽测试的独立保护。
本机日常开发环境不直接运行无保护逃逸/耗尽夹具。

若这条候选也无法提供足够证据，则停止，不偷偷回退为 RSS monitor 或扩大信任。
若用户坚持“普通 macOS 进程直接运行任意 Mach-O、无 VM、无特权、完整硬资源
保证”全部同时成立，目前应如实记录无已验证方案，不能承诺自研必然补齐。

## 7. 来源与本轮验证

检索/读取于 2026-09-20；Apple 网页可能更新，本机 SDK 为 Xcode 27.0 / SDK 27.0。
这是只读接口/源码分析，非候选运行、正式验收或第三方安全审计。没有 kernel/
container 镜像下载、VM/扩展启动、签名注册、hosted CI 或产品 gate 验证。

- [S1 Apple helper](https://developer.apple.com/documentation/xcode/creating-enhanced-security-helper-extensions)
- [S2 AppExtensionProcess](https://developer.apple.com/documentation/extensionfoundation/appextensionprocess) 与 [invalidate](https://developer.apple.com/documentation/extensionfoundation/appextensionprocess/invalidate())
- [S3 memorySize](https://developer.apple.com/documentation/virtualization/vzvirtualmachineconfiguration/memorysize)
- [S4 VM stop](https://developer.apple.com/documentation/virtualization/vzvirtualmachine/stop(completionhandler:)) 与本机 VZVirtualMachine.h / VZMacOSConfigurationRequirements.h
- [S5 cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)
- [S6 Containerization README](https://github.com/apple/containerization/blob/d29e00a7c17cb0f454c8945bd3a6bc447e233cfb/README.md)
- [S7 VZ manager](https://github.com/apple/containerization/blob/d29e00a7c17cb0f454c8945bd3a6bc447e233cfb/Sources/Containerization/VZVirtualMachineManager.swift) 与 [instance](https://github.com/apple/containerization/blob/d29e00a7c17cb0f454c8945bd3a6bc447e233cfb/Sources/Containerization/VZVirtualMachineInstance.swift)
- [S8 guest cgroup](https://github.com/apple/containerization/blob/d29e00a7c17cb0f454c8945bd3a6bc447e233cfb/vminitd/Sources/Cgroup/Cgroup2Manager.swift)
- [S9 Package.swift](https://github.com/apple/containerization/blob/d29e00a7c17cb0f454c8945bd3a6bc447e233cfb/Package.swift)
- [S10 Wasmtime ResourceLimiter（48.0.2）](https://docs.rs/wasmtime/48.0.2/wasmtime/trait.ResourceLimiter.html)
- [S11 FFmpeg VideoToolbox](https://ffmpeg.org/doxygen/7.1/group__lavc__codec__hwaccel__videotoolbox.html)
- [S12 Apple vsock](https://developer.apple.com/documentation/virtualization/vzvirtiosocketdevice) 与 [virtualization entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.virtualization)

本机 SDK 头文件显示所需公开 API 存在；本轮未调用 isSupported/validate/start，
不宣称该主机已有可运行 VM 或签名条件。第三方库版本仅固定引用，未加入依赖。

交付检查：`./scripts/verify-repository.sh docs` 的 27 项测试、Rust fmt 和
`git diff --check` 通过。本报告已加入当前路线一致性及负向断言。它们检查文档
合同，不是 VM/扩展/媒体功能或资源资格验证。未修改生产代码或提交 Git。
