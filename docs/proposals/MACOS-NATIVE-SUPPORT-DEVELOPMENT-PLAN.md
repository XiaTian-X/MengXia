---
title: "macOS 原生支持补全：可行性、执行边界与后续开发规划"
document_role: "Development roadmap and evidence; not an implementation start gate"
status: "RESEARCH_RETAINED_REVIEWED_NATIVE_DIRECTION_ACCEPTED"
version: "0.1.7"
date: "2026-09-20"
repository_head_reviewed: "4fcf3470a6a98ad09feaf12152cee8c69740e467"
---

# macOS 原生支持补全开发规划

## 1. 结论与本次范围

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


用户决定自行补齐原生支持不足的部分，本轮首先完成全面分析与开发分阶段规划。
方向是**复用系统强制隔离，自己实现项目专属控制层**，不是自研内核、安全运行时、
通用容器平台或密码协议。不存在“自己实现就必然补齐 OS 能力”的承诺。

本计划不改变 ADR-0020 的 macOS 内置功能优先交付，不启动 Ubuntu，不自动选择
Wasm、VM、特权 helper 或 TRUSTED_NATIVE，也不接受尚未界定的宿主 DoS 风险。
第三方 Native 安装、激活、执行仍延后并禁用；研发规划不等于提前纳入首版发布。

原规划轮仅修改文档、复核证据和运行文档验证；后续获准的 R0 轮只新增七个
`scripts/native-research` 研究文件并记录结果。两轮均没有产品实现、task-start、
新增依赖、签名身份选择、系统注册、CI tuple 变更或生产执行授权。下文后续阶段和
文件集合是计划，不是精确实施白名单；每个实施包只冻结实际消费的合同和验收。

结论：R0 有界研究已完成并按预设出口收敛为 `INCONCLUSIVE`；执行后端仍未合格。
纯基础开发不依赖其完成，但真实内置执行同样不能绕过资源与隔离验证。TASK-012
继续暂停，R0 的开发观察不得作为 R1、BUILTIN_EXECUTION 或生产执行资格。

## 2. 仓库现实与差异分类

基线是上述 HEAD 加现有未提交规划改动，不是干净 checkout。已有改动包含入口
文档、TASK-012/双版本/内置路线、生命周期记录和文档回归测试；本轮不覆盖它们，
不把所有工作树变化归为本轮成果。历史 DONE 与 CI 证据保持原 attribution。

| 观察 | 分类 | 后续处置 |
|---|---|---|
| package 已有纯 Manifest、摘要、RuntimeDependency，target 固定为 `aarch64-apple-darwin` | FACT / 可复用 | 声明不是安装、受控导入、签名验证或实际镜像身份证明 |
| security 目前是 PermissionDiff，不是完整 Grant/Lease/Broker | EXPECTED_GAP | TASK-013 补齐，纯比较结果不是授权 |
| host 已有 caller-supplied streams、有界 session、期限、取消及 TerminationRequired | FACT / 可复用 | 不会自行 spawn/kill/reap；session 关闭不能代替进程结束 |
| platform-sandbox 只有模块边界说明 | EXPECTED_GAP | 不存在需要整体推翻的既有沙箱后端 |
| TASK-011 architecture 测试禁止 host 的进程 API、平台依赖和 Core authority | CONFLICT（未来集成时） | 精确保留协议层禁令，单独授权启动编排模块，不删除整项保护 |
| 普通进程 Mach footprint 限额 API 返回 KERN_NO_ACCESS | FACT / 特定机制不可用 | 不重试提权，不外推为所有原生方案永远不可能 |
| TASK-012 §10.1 用别名/旧手册否定 RLIMIT_AS 的分配限制作用 | SPEC_STALE | 新观察修正依据，不解除总资源阻塞；旧草案接受前必须同步 |
| Apple 扩展可能复用进程，断开连接不保证终止 | UNKNOWN / 与旧候选直搬冲突 | 一个 connection 不是一次 Run 的独占进程，先验证生命周期 |
| 内置、第三方、GPU、任意 CLI 的支持边界不同 | UNKNOWN | 一种 worker 通过不得外推为全部插件通过 |

核对代码：`crates/mengxia-plugin-package/src/{dependency,manifest}.rs`、
`crates/mengxia-plugin-security/src/{lib,permission_diff}.rs`、
`crates/mengxia-plugin-host/src/{session,limits,error}.rs`、
`crates/mengxia-platform-sandbox/src/lib.rs`、
`crates/mengxia-testkit/tests/architecture.rs` 及 TASK-010/TASK-011 测试。
以上花括号为文件清单缩写，不是实施授权。

## 3. 证据与候选：不是已经解决的能力

### 3.1 本轮小型本机探测

重读 tuple：arm64、macOS 27.0 build 26A428、Xcode 27.0 build 27A266a、SDK 27.0。
这不是已接受的生产 sandbox tuple，不能替换历史 hosted attestation。

重跑先前临时研究程序：只限制自身、5 秒 alarm、禁止 core dump；一次 self-exec，
各次 32 MiB 申请不触碰 payload 页面，不访问真实 Library/秘密或其他进程，不提权。
没有建立生产沙箱或执行第三方/恶意二进制。原始结果：

```text
parent_virtual=500512079872 limit=500528857088 setrlimit=0 errno=0
child_virtual=500511604736 inherited_soft=500528857088 inherited_hard=500528857088
raise_hard=-1 errno=1
malloc_32MiB=FAILED errno=12 payload_pages_touched=0
mmap_32MiB=FAILED errno=12 payload_pages_touched=0
```

另一次自身顶层区域元数据枚举得到 57 个区域、virtual_bytes=500511490048：

| 当前 protection | 合计字节 |
|---|---:|
| NONE | 439214456832 |
| READ | 61122002944 |
| READ/WRITE | 175013888 |
| READ/EXECUTE | 16384 |

观察仅证明被测分配路径的虚拟地址限制有效，并在一次相同程序 exec 后保留。
没有证明不同镜像、既有保留区、COW、文件/共享映射、Mach/IOSurface/GPU、宿主代办
及内核记账的全部边界。虚拟空间约 466 GiB，不是用了 466 GiB RAM；权限统计也
不是驻留量/完整 footprint。不能把“启动 VAS + 预算”直接当物理内存硬上限。

研究产物仅供追溯，不是 repository/release 依赖：

- 临时源码 `/tmp/mengxia-native-survey.W3HoN6/native_probe.c`；
- 源码 SHA-256 `377a39f0bde175cd51cb550fa6abbce6bd0cd37bf5be27d6ad024169bc02f445`；
- 二进制 SHA-256 `13f9d9c734c6c01a4d1717bcd4fabedfdfd7a6f067a055aebdea018575c130e7`；
- 本轮运行该二进制的 `reexec`、`inventory` 模式。重复构建需核对源码并记录当次
  SDK/指纹；临时目录可能消失，不能声称已有长期可重复夹具。R0 应纳入可审查源码、
  构建命令、版本化原始结果和反例。本轮没有把 C probe 添加进项目实现。

Apple 公开 XNU 的资源设置和 exec 路径有地址限制处理，支持继续研究；公开 main
不是运行内核的精确源代码证明。
[资源设置](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_resource.c)、
[exec](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_exec.c)。

### 3.2 只筛查两个原生候选，不同时建设多个后端

| 候选 | 借鉴/复用 | 淘汰性问题 |
|---|---|---|
| ExtensionFoundation + Enhanced Security helper | 官方独立扩展、受限进程、宿主代理 | CLI/daemon 部署和注册；内置/外部扩展的身份、独占运行、取消、资源覆盖 |
| 最小 native launcher + Seatbelt | 先沙箱后加载、显式权限、私有通信 | 接口支持性、初始化窗口、硬资源、镜像绑定、系统升级维护 |

Apple 候选优先筛查，不是默认胜出。官方说明可能连接已有扩展进程，`invalidate()`
不是自动终止；不能套用原候选 parent/child/session/reap 假设。未核实到满足项目
要求的公开硬内存配额配置承诺。“扩展无需 UI”也不等于无需 app bundle/签名/注册。
[扩展模型](https://developer.apple.com/documentation/extensionfoundation/adding-support-for-app-extensions-to-your-app)、
[Enhanced Security helper](https://developer.apple.com/documentation/xcode/creating-enhanced-security-helper-extensions)。

Chromium 借鉴点是最小启动器在复杂框架和静态初始化前建立沙箱。其文档也涉及
非公开头文件接口及未正式文档化的策略语言，不能以他人使用代替本项目的支持性
和安全证明；若需修改已有接口支持性要求，先明确 ADR，不得偷换。
[Mac Sandbox V2](https://chromium.googlesource.com/chromium/src/+/main/sandbox/mac/seatbelt_sandbox_design.md)。

VM/Wasm 只保留为需另行选择的备选，不引入库、不启动 guest/Ubuntu。
自定义 malloc/SDK、RSS 轮询、一次分配失败均不能独立闭合不可信 Native 硬限制。

## 4. 推荐架构与自研边界

这是责任建议，具体布局和 public API 在 R1 冻结，不是已接受的新依赖图。

| 层 | 自研/组合责任 | 不能推断或承担 |
|---|---|---|
| Package / custody | 闭合依赖、受控导入、摘要/签名、不可替换对象及启动绑定 | manifest/publisher 字符串不是认证；签名不是业务授权 |
| Platform backend | OS policy、启动前限制、进程身份、终止证据、平台事实 | 不自行签发 Grant/Lease，不解释 Project 权限 |
| Supervisor | 预算预留、进程与协议组合、绝对期限、撤销、唯一终止所有者、回收 | EOF/取消成功/断连不是执行终止 |
| Private protocol | 复用 TASK-011；按版本规则增加必要能力消息或有界桥接 | Ping/Shutdown 通过不是媒体调用/Broker 已实现 |
| Policy / Broker | typed operation、Run/digest/caller/grant 绑定、预算及审计 | 不提供任意 shell/URL/body/path/dylib 或秘密旁路 |
| Run / adapters | intent/effect/observation、受控工具、产物校验、恢复 | 退出 0 不等于输出可信；部分成功不是原子完成 |

共同原则：

1. 必须在不可信首次执行、静态初始化和依赖加载前建立所需边界，不接受 ready 后补沙箱。
2. 资格 manifest 绑定 tuple/backend/policy/helper/测试/源码；每次启动仍验证实际对象、
   身份、权限和预算。资格研究或插件自报不能直接构造产品全 ENFORCED evidence。
3. 清理无关 FD、环境、HOME、Client/Admin 端点和凭据。相同 UID/XPC 连接存在不等于
   插件身份；不声称防御任意已控制 owner 账户的攻击者或被攻破的内核。
4. executable 之外的 dylib/rpath/bundle 资源及签名必须纳入闭合加载模型；更新、
   重签名和注册后的运行对象仍须绑定审核对象，覆盖路径/对象替换竞争。
5. Swift/XPC/Seatbelt 不进入 domain/store/Recipe。仅在实测需要时增加最小 shim，
   单独审查 FFI 所有权、线程、异常和取消；不把新 unsafe 放进已禁止它的纯模块。
6. 复用开源前审查来源、维护、许可证与依赖。借鉴经验不等于无条件复制代码。

## 5. 资源与可用性覆盖矩阵

这是待验证清单，不是已有 enforcement。产品数值在消费它的实施 gate 接受；
性能 SLO 经基准确定，不能用本轮研究数值充当正式默认值。

| 资源 | 控制方向 | 必须拒绝的错误证明 |
|---|---|---|
| worker 内存 | 验证系统硬限制，明确 VAS/footprint/RSS/共享/压缩记账与单位 | 设置成功、单次分配失败、自定义 allocator、RSS 超限后 kill |
| 宿主代办内存 | 分配前预留、有界流式复制、全局/每 Run 并发与缓冲池 | 只限制 guest；多个合法请求累积耗尽宿主 |
| CPU / 墙钟 | CPU 累计限制与宿主绝对 deadline 分别证明，覆盖 exec | 把墙钟当 CPU 配额，重连/exec 刷新预算 |
| 进程/线程 | 限制未声明执行、子进程、线程与系统代启动，按实际模型收尾 | UID 的 NPROC 当私有树配额；只看 PID/进程组 |
| 输出/临时磁盘 | 总字节、文件数、元数据及并发预留，验证实际强制边界 | FSIZE 单文件上限当目录总配额；扫描后删当硬限额 |
| FD / Mach ports / IPC | 闭合继承、连接数、消息排队及解析前大小限制 | 只限制 protobuf payload，遗漏 XPC/系统代分配 |
| 日志/产物 | 有界排空、总量、反压/终止，输出数量/类型/摘要校验 | 无限 drain；停止读取后死锁；跟随输出 symlink |
| GPU / IOSurface / 服务 | 逐能力声明访问、记账、取消和数据共享范围 | CPU/地址上限覆盖 GPU；放开全部 IOKit/Mach |
| 网络/付费 | 默认无直连，语义 Broker 的字节/并发/次数/费用与身份约束 | 域名名单当底层断网；通用代理旁路；重试重复付费 |

可写 FD 可能绕过 Broker 写入计数，必须另有实际总量边界；若改为受控写入协议，
需证明 FFmpeg/插件兼容，不是文档换一个名称就解决。第三方插件不得自行 fork
FFmpeg；推荐宿主以独立受控 worker 执行，累计消耗同一 Run 预算并共享撤销/审计，
具体合同由集成 gate 确定。这不删除 TASK-014 或放宽 TASK-012 现有执行禁令。

首批 CPU-only 夹具仅用于缩小验证变量，不永久删除 GPU 或宣称性能达标。
必要维度不能闭合时保持 UNKNOWN/不可用，不以另一维度的保护代替。

## 6. 分阶段开发

R0–R6 是计划内阶段标签，不是新的稳定 TASK/AC/TEST ID，不改变既有编号和 DONE。
每阶段有决策出口，不要求未来实现完成才允许编写当前实现。

它们不是要求所有工作串行等待的任务表：

| 工作线 | 先后关系 | 汇合点 |
|---|---|---|
| 产品基础 | PLAN_FOUNDATION → BROKER_FOUNDATION | 提供 R2 所用的纯合同；可先于或穿插 R0/R1 完成 |
| 原生机制 | R0 → R1 → R2 | 合格内置 profile 才能满足 BUILTIN_EXECUTION |
| 内置集成 | 两者满足后进入 R3 → R4 | 真实工作流与首版发布 |
| 第三方接入 | 完整第三方资格及持久化/Admin/Run 具备后进入 R5 | 不把内置资格当第三方资格 |

R6 是从首次执行发布起持续适用的维护义务，不是等 R5 完成后才处理升级。

### R0：有界研究已完成，汇合为 INCONCLUSIVE

精确设计与完成证据见 [R0 研究设计](MACOS-NATIVE-R0-RESEARCH-DESIGN.md)：R0-A
已实现无需签名/注册的有限自进程探针，12 个 case 各三次均完成且无未决；R0-B
只完成不改变系统状态的 SDK、签名身份和当前 Seatbelt 工具 tuple 核查。研究没有
使用真实 Library、秘密、外网、用户插件或产品代码，结束后 authority 仍为 NONE。

优先查内存/总磁盘与 Apple 身份/进程复用/终止/CLI 部署；另一候选查前置限制和
加载时机。只比较两个候选，不先建设完整 SDK。小型自进程探测不依赖已完成沙箱，
但这绝不授权无隔离地运行真正 hostile fixture。耗尽/逃逸试验只在已有适当独立
保护的专用测试环境下进行；缺少签名/注册条件只阻塞相关实验，不擅自更改系统。

研究包使用下列顺序收集最小证据；当前完成状态如下：

| 优先级 | 实验问题 | 必须输出 |
|---|---|---|
| 先决 | 普通权限能否对内存与总写入建立实际强制边界？ | `PARTIAL`：相对 AS 拒绝被测分配；FSIZE 反证目录总配额；完整物理/共享/GPU/聚合写入仍未知 |
| 先决 | Apple 扩展能否保持无 UI 产品入口并可靠运行/终止？ | `NOT_RUN`：SDK 接口存在但无签名身份，且未授权注册/启动/系统状态变化 |
| 其次 | 包及加载闭包能否绑定实际运行对象？ | `NOT_RUN`：没有真实候选启动对象，不能用源码/工具 hash 代替运行镜像绑定 |
| 其次 | 现有私有协议能否保持全部有界语义？ | `NOT_RUN`：未选择后端，无 XPC/pipe 实际桥接实例 |
| 汇合 | 最小真实媒体功能能否在候选限制下工作？ | `NOT_RUN`：保护前提未闭合，未运行任何媒体 executable 或 hostile fixture |

只有先决机制足以支持安全实施，才在 R1 选择 backend；后面的正式 hostile 和
完整 FFmpeg 工作流在 R2/R3 实现验收。若研究需要运行原生媒体程序，仍先建立
与该夹具相适应的独立限制和有限预算；本轮规划并未批准直接运行任意工具。

出口：`R0_CONVERGENCE: INCONCLUSIVE`。现有证据不能选定后端，也没有证明所有候选
必然失败。无新平台条件不重复同一 R0-A；后端扩张停止，纯基础开发继续。只有一个
候选获得精确受审实验范围、必要签名/注册或 Seatbelt tuple，且能验证资源、身份与
生命周期后，才可重新进入 R1 评审；不得做无隔离“临时可用版”。

### R1：选择一个后端，冻结最小实施合同

依赖 R0 可行性，不要求已经存在完整 hostile-suite PASS。提交一个后端 ADR 和
bounded start：候选 tuple、启动/身份/进程模型、资源语义与数值、精确代码/FFI/
依赖/测试/文档范围、失败与清理语义、稳定验收义务。

对照现有 TASK-012 草案逐项保留/替换并同步 canonical 文档，不维护两套有效合同。
extension bundle 如需新包格式、完整 custody、注册后镜像绑定和 XPC 桥接，先闭合；
若与单进程/单 Run 不兼容，拒绝或明确接受合同变更，不共享进程却声称独占隔离。

出口：仅授权 test-only/non-admitting 的真实路径实现；未定安全参数不进入实施。
不是把所有未来测试预填 PASS，也不伪造签名身份、Provider 或限额。

### R2：受控导入、启动、监督与完整执行资格

依赖 R1 和所用纯 Broker 合同，沿用 TASK-012 拥有者边界。实现 custody、限制、
通道、supervisor；覆盖握手前后失败、driver drop、断连、超时/撤销、重复 exec、
PID 重用、父进程死亡、启动中崩溃和退出未确认。Rust task join 不自动管理进程。

退出未确认时不把实例/预算视为可复用，不发布输出；隔离或持有相关资源、拒绝
后续 admission 并记录有界错误。重启不能凭旧 PID 杀进程。文件/网络/IPC/进程/
资源必要维度均需真实负向及故障测试，临时 probe/mock 不替代最终执行路径。

出口：qualification manifest 与最终代码/policy/工具对应，适用 formal/第二 UID/
供应链证据完成。只通过内置 profile 则仅满足 BUILTIN_EXECUTION，不标完整
TASK-012 DONE，不宣告第三方可用。完整 suite 是实现后的验收，不是写代码的前提。

### R3：持久化 Broker 与内置媒体/Run 闭环

遵循规范 §0.7，不另建相反依赖图：

1. PLAN_FOUNDATION 与 BROKER_FOUNDATION 是纯合同，原生研究不阻塞它们。
2. BUILTIN_EXECUTION 消费 R2 合格的适用 profile。
3. BROKER_PERSISTENCE 依次拥有 0003/0004；第一项迁移实施前审查与后续 0005 的绑定。
4. FFMPEG_INTEGRATION 完成受控媒体、产物、取消和畸形输入验证。
5. RUN_INTEGRATION 加入 0005，完成真实 Run/lease/audit、恢复与 CLI/Core API。

不得伪造 Run、提前发产品 lease、建占位迁移或让纯合同代替持久化。
PLAN_FOUNDATION 的 candidate/canonical typed-catalog 输出仍由其启动草案选择，
不伪造 provider/plugin/package digest 或可运行权限。

出口：一个用户可用、可取消、可恢复的真实内置媒体工作流，不仅是 Ping 演示。

### R4：凭据、受控外发与其余首版能力

保留 TASK-016/TASK-017/TASK-018/TASK-019/TASK-020 的顺序及 OQ-004/OQ-005/OQ-006/
OQ-010。真实上传满足适用 TASK-021 Rights/classification；unknown 默认拒绝。
本地 CLI 也需合格执行 profile。TASK-022 destructive/retention gate 不变。

出口：首版接受能力逐项可用，通过适用 TASK-023；不因原生研究默默删除 Provider、
Rights、CLI/API、恢复、审计功能，也不承诺所有能力同时完成。

### R5：第三方 Native 产品接入

默认在内置路线稳定后接入；提前开放须明确调整发布范围。研究/SDK 准备不等于开放。
入口同时满足完整第三方 profile 的 TASK-012、TASK-013 持久化能力、OQ-010 Admin
机制及真实 Run 集成；涉及秘密/外发还需 TASK-016 与适用 Rights。内置 PASS、
publisher 签名或外部扩展发现都不是安装/激活授权。

实现安装/更新/审批/激活/撤销/终止、exact-digest 更新策略、最小 SDK 与一个示例；
明确包格式、协议/能力版本、支持范围及 unsupported 错误。更新不能继承不匹配 grant。

出口：真实第三方 fixture 的越权、错误依赖、跨 Run lease、撤销竞争、资源耗尽、
恶意输出与升级矩阵通过，再验收新增能力适用的 TASK-023 后发布。不把这些条件
倒灌为 R0 或纯基础前提，也不暗示任意 Mach-O/GPU/CLI 均受支持。

### R6：系统升级与平台维护

按 tuple/profile 独立资格管理；OS/SDK/backend/policy/image 变化做影响分析。
开发兼容性 PASS 不是安全资格。未合格执行 profile 拒绝启动，但不因此禁用不依赖
它的资产管理功能；真实共享基础缺陷仍阻断实际受影响范围。

保持 ADR-0015/ADR-0016 的 CI 去重、工具维护、正式 attestation 和合并证据。
runner 缺签名/注册/OS 能力不得 fake PASS；启动 gate 明确可重复测试的 runner/设备
与秘密保护。普通开发账户不作耗尽攻击靶机；必要能力无正式证据不得发布。

Ubuntu 仍在用户既定 macOS 完成条件之后独立接管；R5 与 Ubuntu 的相对优先级届时
安排，不自动启动两者。保留共用语义，不提前建设 Linux 适配/CI。

## 7. 兼容性与任务归属

| 区域 | 计划处置 | 回归要求 |
|---|---|---|
| TASK-001 至 TASK-009、0000/0001/0002 | 保持已交付合同，具体共享缺陷才单独修复 | 既有 workspace/迁移/恢复/第二 UID，不仅新测试 |
| TASK-010 package/schema/digest | 保留现有对象；bundle 如需新语义则明确版本化 | 保留旧 golden/拒绝行为，旧 schema 不静默接收新权限 |
| TASK-011 proto/session | 保留有界协议；XPC 不可无损桥接时明确演进 | 版本/帧大小/复制预算/队列/EOF/deadline/身份回归 |
| platform-sandbox / platform-fs | 分别拥有限制和 custody，FFI/unsafe 最小化 | 错误镜像、路径替换、继承能力、失败释放与不越权 |
| host / architecture tests | 协议层仍无进程权限；R1 确定编排模块位置 | 仅授权模块允许边改变，保留纯 crate 禁止反向依赖的负向测试 |
| security/ports/app/store | 纯策略先行，真实效应按 R3/R4/R5 | package/security 不增加 IO，不跳迁移、不混淆 Client/Admin/Plugin |
| testkit/scripts/CI | 启动前纳入稳定 ID/映射及 scoped completion 所需文件 | 旧测试兼容、helper、生命周期验证器和 CI 脚本不在最后才补授权 |

TASK-012 拥有执行资格，TASK-013 拥有产品安装/授权/Lease/audit，TASK-014 拥有
FFmpeg，TASK-015 拥有 Run，TASK-016 拥有秘密/完整外发组合。terminal AC 归属以
规范为准，例如 TASK-012 不独占 AC-020/AC-023 的终验。不得简化成一个“Native 已支持”
布尔值并让后续消费者跳过各自证据。

## 8. 验证与减少返工

每个实施包启动前只闭合其实际需要的能力/攻击模型、来源/tuple、有限预算、唯一
状态所有者、失败语义、精确文件和验收归属。缺少的稳定 AC/TEST 先在 gate 定义，
命令在实施中实现，完成与消费前机器校验，不要求验收代码在启动前已存在。

验证分纯合同、真实平台负向/故障、用户工作流三层。覆盖并发撤销、暂停/时钟变化、
重试/重启、同镜像 exec、父进程死亡、输出替换/部分写入、消息洪泛、错误 grant/digest
及升级后资格失效。资源攻击夹具先有自身及宿主保护，不运行危险无限循环补证据。

只有新证据、真实冲突或范围改变才重开已决问题。阶段开始与验收集中同步文档，
不因每个内部函数重写全局规范；安全/公共合同变化仍及时记录。未测量不承诺工期、
峰值内存、吞吐或支持 tuple。

v0.1.0 规划验证记录（2026-09-20，规划工作树，未提交）：

- `./scripts/verify-repository.sh docs`：27 项通过（traceability 6、naming 4、
  CI orchestration 5、CI evidence 12）。首次运行发现本轮文档升版未同步台账；
  同步 `task-lifecycle-records.toml` 的三个版本值后重跑通过，未更改测试或任务状态。
- 新计划的 current-action 唯一性、四个入口的计划引用及目标文件存在性：命令检查通过。
  这是本次补充检查，不声称既有测试已永久覆盖新文档的全部规划语义。
- `git diff --check`：通过；新未跟踪文档另做 `git diff --no-index --check`。

这些是 v0.1.0 规划验证，不是 sandbox conformance；该轮没有运行完整 workspace/formal、
真实第二 UID、hosted CI、签名扩展或危险资源压力测试。没有改变生产代码、既有
测试、协议、迁移或依赖；该轮文件范围为新计划、AGENTS、DECISIONS、REVIEW、PLAN
及文档版本台账。工作树中其他差异属于先前工作，不在该轮改动归属内。
v0.1.1 仅细化 R0 入口与下一步。v0.1.2 记录 R0-A 实现/实机证据、R0-B 只读
preflight 和 `INCONCLUSIVE` 汇合；精确命令、digest、限制与未测项见 R0 研究设计 §9。

v0.1.3 记录研究工具审查修复：初版存在清理/继承/证据/来源/期限及负例缺陷。
采用纠正后的 schema-2 证据与故障回归，精确结果见 R0 研究设计 §9.4；初版
36 次正常观察和 11 个负例不作为纠正后行为的验收。修复不改变 R0 汇合或产品路线。

## 9. 当前下一步

ADR-0021 和 [审核准入开发计划](REVIEWED-NATIVE-PLUGIN-DEVELOPMENT-PLAN.md) 已接替
旧的硬内存补全路线。审查该计划 §5 后，可单独启动纯准入判定基础；本文件不是新
profile 的实施白名单。后续实现仍须选定一个能提供保留隔离边界的原生候选及精确
预算，但不再要求先证明完整硬物理内存限制。必要重构可纳入其精确 gate。

### 历史下一步（已被 ADR-0021 替代）

执行边界比较已经完成，当前详细结论与互斥选择见
[执行边界候选比较](MACOS-NATIVE-EXECUTION-BOUNDARY-COMPARISON.md)。若不要求
Darwin 插件兼容，推荐优先验证原生 macOS host + 最小 Linux VM worker；这仍是
条件推荐，不是已接受 backend/VM/Ubuntu 启动或硬资源验收。用户的兼容性问题
尚不是架构授权；选定一条后再审有界实验，不能直接下载镜像或同时开发两套。

用户已明确当前优先解决原生可行性。
[第一批](MACOS-NATIVE-R0B-001.md)与[第二批](MACOS-NATIVE-R0B-002.md)有限实验
已完成：AS/DATA 不能约束已有映射的物理增长，替代 memorystatus 入口也拒绝。
下一步按第二批 §5 比较执行边界候选；没有新机制不重复同一探针，不直接扩建
后端，也不转向 TASK-015。原 R0 汇合的
`INCONCLUSIVE` 保留为历史证据，新研究不直接恢复 TASK-012 生产实施。
[R0 研究设计](MACOS-NATIVE-R0-RESEARCH-DESIGN.md) 的 R0-A 无需重复建设。
第一批选择无需签名/注册的 Seatbelt 机制筛查，没有同时建设多个后端。

只有新的平台条件和另行批准的单一候选实验能够重新进入 R1；届时必须验证完整
资源覆盖、运行镜像身份、独占生命周期和终止确认。不能只验证通信便宣布路线可行，
也不能把用户“自行补齐”的方向选择理解为任意安全取舍或生产执行已获批准。
