---
title: "MAINT-003 工具链兼容与必要安全维护规划"
document_role: "Proposed maintenance plan"
status: "ACCEPTED_IN_PROGRESS"
version: "0.2.0"
date: "2026-09-13"
repository_head_reviewed: "b2fa8d52580a58f014da97e9473e647681911389"
---

# MAINT-003 工具链兼容与必要安全维护规划

## 1. 目标、当前状态与授权

2026-09-13 复审修订：用户要求全面审查后实施；ADR-0016 接受有限维护范围，
MAINT-003 为 IN_PROGRESS / MAINT_003_TOOLCHAIN_ONLY，product authority 为 NONE。
下文原规划/验证记录保留为历史；发生范围冲突以本修订和 ADR-0016 为准。
完整 developer 保留 target/debug 布局，以新鲜环境指纹触发 ACL/SQLite build.rs
重建；只对候选使用空构建目录。原因是现有 CLI 门禁直接消费 target/debug。
首版使用独立可信摘要固定的官方 cargo-deny 二进制，不自行实现源码安装器。
拟执行清单与安全事件文件均属于 code CI。持续修复代理为 NOT_ENABLED，不在本次
部署；安全报告须明确逐源 UNKNOWN，不声称全自动接管或全工具无漏洞。

用户要求在 MAINT-002 完成后规划工具链维护：正常系统更新与部分开发工具更新应尽量
不打断开发；可能改变兼容性的工具更新必须重点验证；适用的安全更新必须处理。
日常检测、诊断、准备修复和验证尽量由代理承担，重大变化再交由用户决策。

原 v0.1.0 交付时 implementation authority 与 product authority 均为 NONE。
现由 ADR-0016 与实施计划中的独立启动记录授权 MAINT_003_TOOLCHAIN_ONLY；
product authority 仍为 NONE。本提案不替代启动记录。MAINT-002/ADR-0015 已
完成，不能沿用其撤销的临时授权。后续产品任务不因本规划增加前置依赖。

“不打断”定义为：在已支持的 arm64 macOS 开发范围内，环境仍满足必要安全与 ABI
条件时，不因旧版本字符串、全局工具被替换、过期缓存或错误诊断而拒绝开发；必要
工具缺失时由明确的准备入口补齐，环境变化后自动重新验证受影响的构建边界。
真实 ABI 破坏、未接受许可证、缺少安全补丁或 OS 移除必需能力，可能使相关构建
暂时不可用；不能承诺任意未来系统无条件兼容，也不能以继续开发为由输出错误 PASS。

当前只规划 macOS 开发环境。NAS、数据库位置、存储后端、Linux/Windows/Intel 移植、
远程通信、最低发行系统与产品自动更新均不在范围内，用户无需现在决定存储场景。
最低 OS 支持仍由原平台/沙箱决策及 TASK-023 的发行验证确定。

## 2. 基线与差异分类

基线为 MAINT-002 后的 main `b2fa8d52580a58f014da97e9473e647681911389`，包含 PR #7
的 CI fixture 修正。MAINT-002 的历史 PR/main 验收仍是 run 34676854969/34677363307，
不能把这些旧运行冒称为本规划或未来工具链实现的验证。

| 观察 | 分类与证据状态 | 规划处置 |
|---|---|---|
| Developer 已允许安全的新 Xcode，Attested 精确比较工具身份 | FACT；platform build.rs 的 BuildClass 分支、ADR-0013 | 保留双层证据；本地变化不自动要求 CI 更换工具 |
| ACL build.rs 的 rerun 规则主要跟踪源文件及构建类别；SQLite build.rs 同样没有完整跟踪外部 Xcode 切换 | UNKNOWN / 静态风险，尚未在真实工具升级中复现 | 先建立热缓存后切换工具的受控测试，再决定最小修复；不是已证实的产品损坏 |
| verify-toolchain-candidate.sh 仅 cargo check 后比较 Xcode/SDK 版本；不保证重跑 ABI，版本相同不证明字节相同 | REPO_STALE / 诊断证据不足 | 新鲜环境检查与干净候选验证；兼容结果和正式认证状态分别输出 |
| candidate 把所有 cargo check 失败统一归为 UNSUPPORTED | REPO_STALE / 错误分类；缺依赖、源码失败都可能触发 | 分离准备失败、源码失败、环境不兼容和无法验证 |
| check-supply-chain.sh 默认从 PATH 找 cargo-deny，强制 0.20.2 | FACT；全局工具更新可导致本地验证被拒绝 | 隔离安装当前固定工具，保留精确版本和来源验证 |
| Cargo 普通版本更新会提出冻结合同未同步的候选；PR #3 仍开放 | REPO_STALE / 维护策略与使用需求不匹配 | 关闭 Cargo 普通版本 PR 生成，保留安全更新；PR 关闭是实施阶段明确的仓库操作 |
| RustSec/cargo-deny 不是 Apple 系统、Xcode、rustup、Action runtime 的完整公告源 | EXPECTED_GAP / 工具安全覆盖缺口 | 按工具列出公告来源、适用性和处理证据，不能只用 cargo-deny 宣称全部安全 |
| 当前多个测试与 TASK-010 全锁摘要固定历史依赖快照 | FACT；依赖更新需同步合同 | 本阶段列出安全更新影响清单；不批量删除断言或重写历史任务 |
| MAINT-002 已拆分 Fast、formal-native、shared supply、second UID、Merge gate | FACT / ADR-0015 | 接入现有入口和汇总，不恢复双份完整供应链或递归全量验证 |

2026-09-12 的只读主机观察：arm64 macOS 26.6.2 / 25G83，系统选择
/Applications/Xcode.app/Contents/Developer，Xcode 26.6 / 17F113；rustup 报告
repository-local Rust 1.98.0；cargo-deny 0.20.2。这是瞬时事实，不是兼容范围或
当前无漏洞的证明。本次没有替换 Xcode、安装工具或重跑全部产品 CI。

## 3. 更新决策规则

| 事件 | 必需动作 | 是否改变项目基线 |
|---|---|---|
| macOS 安全补丁或正常更新，必需能力仍兼容 | 检测 OS/build 变化，刷新环境证据，运行受影响的能力/构建检查 | 通常无需；不因 OS 字符串变化重写已完成任务 |
| 本地 Xcode/SDK/clang 更新 | 检查真实来源/路径/权限/身份，重新构建 ACL 与 bundled SQLite 等原生产物，运行兼容回归 | 本地兼容即可；Formal 工具仍可用时无需重新认证 CI |
| 全局 Rust/rustup、cargo-deny、protoc 被更新 | 优先使用项目指定工具；验证实际解析结果，缺失时走准备入口 | 不随全局 stable 或 PATH 自动漂移 |
| Action runtime 停止支持，或原认证工具即将移除 | 建立有官方期限/故障证据的维护候选，提前验证可用替代 | 必要维护，完整检查后切换精确基线 |
| 已确认影响当前工具/依赖的漏洞，已有修复版本 | 必须进入修复流程；验证安全修复及兼容性后更新 | 是；不能因旧 pin 或任务 DONE 拒绝修复 |
| 有安全公告但尚未判断是否适用 | 优先核实组件、版本、features、执行方式与漏洞条件 | 未核实前记录 UNKNOWN，不自动宣布安全或随意升级 |
| 已受影响而暂无可用修复，或修复版本不兼容 | 停用受影响工具/路径，寻找修复或等效安全替代并保持问题开放 | 不自动退回已知受影响版本；例外须遵守 SEC-020 的有期限 ADR |
| 只有新功能、普通 minor/patch 发布且无需求 | 不创建 Cargo 追新 PR | 否；可在未来真实功能需求中重新评估 |

“安全升级必须”指适用于正在使用的组件和配置的安全修复必须处置，不能无限延期，
不是不经校验执行所有带 security 标签的包。安全修复可以包含兼容性变化，仍须测试。
任何例外不能自动批准；记录受影响范围、缓解方式、负责人、到期日与退出条件。
本规划不虚构小时级响应 SLO。检测时建立并持续跟踪事件，已确认受影响的构建/发行
路径在解决或合法例外前不得获得安全通过；与其无关的编辑、文档和安全测试可继续。

OS 安全更新不应为了维持旧构建环境而被永久关闭。若新 OS 与旧工具组合不兼容，
代理准备受支持工具或隔离的安全验证环境；有漏洞的旧工具不能作为自动回退方案。
OS/Xcode 的管理员安装、许可证确认或重启可能需要用户操作，普通准备命令不得
自行升级系统、关闭系统安全设置或静默接受许可证。

## 4. 开发入口、工具隔离与证据

拟提供 scripts/dev-toolchain.sh，采用固定子命令而非任意 shell 执行器：

- inspect：只读观察环境、来源/版本、安全状态和准备条件；不联网安装、不切换系统设置。
- prepare：按已批准工具清单补齐缺失工具；复用已验证安装；显式允许网络，离线缺失
  报 NEEDS_PREPARATION；不更新产品 Cargo.lock、不覆盖全局 PATH 工具。
- verify：无网络下载的兼容检查；新候选使用独立空构建目录，禁止旧结果代替新证明。
- fast / check / build / test：准备与验证规则下的固定 Cargo/仓库入口，保留增量性能；
  不接收可改变编译器、features、认证类别或测试豁免的任意字符串。

允许开发入口在按用户接受的准备政策执行时自动调用 prepare；纯 inspect/verify、
Cargo build.rs 及离线验证本身不得偷偷下载。首版不安装 rustup 自身或整个 Xcode，
缺少这类宿主工具时提供精确准备说明，后续代理处理，不冒充零交互安装已完成。

项目工具与证据使用现有被忽略的 target/mengxia-tools/、target/toolchain-evidence/；
prepare 缓存清空后应可恢复。Rust 使用现有 rustup 共存工具链并显式核对实际编译器；
不能仅信 rust-toolchain.toml，因为 +toolchain、环境和目录 override 可能有更高优先级。
cargo-deny 固定版本安装到项目前缀；protoc 保留独立下载、摘要验证和隔离执行路径。

工具清单拟存放 docs/provenance/developer-tools-v1.toml，采用闭合数据字段：工具 ID、
版本、平台、官方来源及校验方式；不包含任意命令、任意认证开关或测试豁免。它属于
机器执行输入，始终走 code CI；不能放进 docs/spec 的纯文档白名单。
现有 rust-toolchain.toml、macOS active provenance 与协议 provenance 各自仍为原
工具的权威来源；统一清单使用受限引用，避免复制多套版本/摘要。检查其一致性。

安装候选必须在可信父目录下隔离准备、校验后原子发布；拒绝链接替换、不可信所有权、
部分安装、错误架构与被篡改缓存。并发 prepare 只允许一个发布者，中断可恢复。
从官方发布或已核对 registry 获取；摘要必须有独立可信来源，不能把下载后的自行
计算值当成来源证明。源码构建需固定版本和工具自身 lock/依赖证据，输出记录摘要；
只验证 --version 不够。不能执行未经验证的缓存二进制来“验证它的版本”。
首次准备不引入新的通用包管理器或高权限 CI action。

开发构建始终是非正式证据；正式环境继续运行完整 preflight 和 ADR-0015 汇总。
开发者本机无需具有与 hosted XIP 完全相同的 clang 字节。

## 5. 环境变化与增量构建正确性

### 5.1 环境证据必须覆盖什么

记录 OS/version/build、架构、实际 Rust/Cargo 版本、逻辑/规范 Xcode 路径、SDK、
Apple 编译器身份、clang/libtool/关键头文件摘要、相关权限与来源验证结果、构建类别、
检查器版本、源码/锁及 active provenance 摘要。观察值不是信任根；版本相同、路径
相同或 mtime 相同都不能独立证明工具未变化。验证过程中身份变化应失败并重试。

compatibility 与 attestation_match 分开：例如兼容的新 Xcode 可为
compatibility=PASS、attestation_match=NO；这不阻止本地开发，也不批准正式认证。
未知字段、重复字段、截断输出、解析失败不能退回宽松默认。

### 5.2 先证明缓存问题，再选最小实现

先建立“旧工具完成构建 → 仅改变工具选择/内容 → 再次构建”的受控夹具，记录 build.rs
是否实际执行。覆盖 ACL shim 和 bundled SQLite；只刷新 ACL 不足以避免混用旧产物。
生产宿主的 Xcode 不用于破坏性注入，伪造工具只在不可进入正式证明的测试夹具中使用。

已接受设计：开发入口在 Cargo 外重新识别环境；身份/检查政策变化时更新原生构建
指纹，ACL 与 SQLite 的 build.rs 显式跟踪该输入并重新编译、触发消费者重链接。
完整开发门禁保留 target/debug 布局，避免现有 CLI 读取错误的二进制。仅独立候选
验证使用新的空构建目录；不删除工作区或全局缓存。执行结束再观察环境，变化则
拒绝本次通过。环境未变化时保留增量，不每次重跑全工作区或下载工具。

不得把 Xcode 目录或指向目录的 xcode_select_link 作为 Cargo rerun-if-changed
输入，否则 Cargo 会递归扫描整套 Xcode。直接 Cargo 只跟踪选定的具体工具/头文件；
系统更新与工具选择变化由外部维护入口覆盖。这一边界必须在开发说明中明确。

直接 cargo 调用也必须有明确语义：build.rs 补齐可观察的外部工具/选择输入 rerun
规则，保证冷构建保持原安全验证；但不能宣称 Cargo 会自动观察所有 OS 全局状态。
正式认可的新环境兼容证据只由上述入口或干净候选验证产生。将该入口接入 Fast、
完整 developer 和候选检查，不允许它仅成为无人调用的辅助脚本。

实现评审须量化每次入口的轻量检查与完整摘要开销；只跟踪已选工具和关键输入，不
扫描整个 Xcode/磁盘。元数据可用于性能提示，不能作为安全状态或正式证明的唯一依据。
跨 run 缓存只缓存已验证工具/构建材料，不缓存安全 PASS；advisory 获取保持新鲜。

### 5.3 诊断结果

| 结果 | 含义与动作 |
|---|---|
| READY | 当前开发条件已验证；另列是否匹配正式认证，不能等同于整仓库 PASS |
| NEEDS_PREPARATION | 缺失工具、未安装固定 Rust、离线依赖缺失；按既定政策准备 |
| SECURITY_UPDATE_REQUIRED | 已确认受影响；阻止受影响工具被认可，启动安全维护 |
| INCOMPATIBLE | 干净验证确认 ABI/API/编译能力不兼容；保留有证据的错误与修复路径 |
| SOURCE_CHECK_FAILED | 源码/锁/普通测试失败；不能归咎于系统版本，也不能报告兼容已通过 |
| UNVERIFIABLE | 来源、权限、工具身份或必要外部证据无法验证；不得报告安全/正式成功 |

诊断允许携带多个独立原因；先做准备/身份检查，再编译，避免通过解析任意 stderr
猜测原因。日志保留有界证据并脱敏；不得上传环境变量、凭据或用户数据目录清单。

## 6. 安全更新与必要维护闭环

1. 保留 Dependabot alerts/security updates、CodeQL、Dependency Review 和当前
   cargo-deny 全部类别。Cargo 配置 open-pull-requests-limit: 0 以停止普通版本 PR，
   不使用可能一并屏蔽安全升级的全包 ignore。GitHub Actions 保留安全和维护候选。
2. 清单必须区分产品依赖、编译工具、验证工具和系统组件。RustSec 覆盖 Cargo 公告；
   Apple 安全发布覆盖 macOS/Xcode；Rust/rustup 与 action 官方发布/公告分别审查。
   路径 vendored SQLite 与编译用工具的依赖也纳入检查，不能只扫产品 Cargo.lock。
3. 复用现有每周扫描触发，安全告警到达时优先处理；不等到月度普通升级批次。
   公开结构化来源可以自动获取；未实现自动解析的官方公告由维护代理核实，并记录
   source/checked_at/applicability。未覆盖的来源标 UNKNOWN，不能编造“全工具无漏洞”。
4. 按工具、公告及受影响基线去重事件。无变化不重复通知、不反复新建 PR；影响扩大、
   新修复可用、临近停服、修复失败或需要用户动作时更新同一事件。
5. 代理准备最小修复候选，记录为何必须升级、来源、旧/新版本、features/依赖差异、
   兼容检查及回退条件。安全更新 PR 初次失败不是结束，要继续处理合同/fixture 或
   兼容问题；严禁删除断言让版本更新强行通过。
6. 当前精确工具/依赖基线保持不漂移；必要升级经记录决策更新到另一个精确基线。
   新认证 manifest 不覆盖历史 manifest。当前依赖清单与历史交付快照的解耦作为
   独立后续子项评审，首版只生成影响清单，不批量改写已完成 TASK 的提案/测试。
7. 每个代码候选仍通过对应完整 PR/main 汇总和安全审查。机器人不得自己放宽门禁、
   批准例外或接受新权限。保留 ADR-0013 的无自动合并规则；未来若改变合并方式，
   单独决策。用户不用反复决定无关紧要的脚本细节。

事件生命周期建议为 DETECTED → TRIAGED → CANDIDATE → VERIFIED → RESOLVED；
不可适用结论附证据，无法修复保持 BLOCKED，例外附期限，均不得用“忽略”代替处置。
每个事件记录受影响对象、公告、适用性、修复目标、执行者、下一步、验证和关闭证据。
此为候选运行记录格式，不与产品生命周期记录混用。

自动创建/更新 PR 的执行位置必须在开启前闭合：普通 PR CI 仍是只读、无 secrets、
无 pull_request_target；不能让未经审查的 PR 内容在具有仓库写权限的任务里执行。
首版维护脚本输出有界报告，由有权限的现有维护代理消费并处理 PR，不在 CI 内新增
通用 AI 执行器。后续常驻代理/通知机制若未配置，必须明确为尚未自动接管，不能仅
凭每周 workflow 就声称无人维护闭环已完成。配置常驻机制需要独立的宿主、凭据和
触发设计，但不妨碍本阶段本地兼容/工具隔离交付。

## 7. 兼容性与恢复策略

先区分环境变化、源码回归、产品依赖变化，分别保存证据。更新 Xcode 要覆盖 ACL
ABI、SQLite build/runtime identity 和涉及的权限/IPC/恢复测试；更新 Rust 要覆盖
编译、Clippy、测试、编译失败 fixture 及 MSRV 决策；更新 protoc 要验证协议生成
字节一致；更新验证工具要测试已知正/负样本与不可用情况，避免升级后漏报。

新工具验证失败时保留之前的工具材料及代码，只有仍安全、仍受宿主支持的旧工具才
可用于恢复开发。不自动回退系统、不降级已迁移数据库、不启用脆弱工具、不删除
Library/CAS。不能假定新版 macOS 一定允许旧 Xcode 运行；无可用安全组合时，交付
明确诊断并使用可验证的安全开发主机/CI处理，相关本地构建暂时受阻。

构建环境支持与成品运行支持分别记录。新 SDK 构建成功不能证明最低 macOS 可运行，
旧工具重建也不能证明新版 OS 的插件沙箱有效；发行兼容仍遵循原 TASK-012/TASK-023。

## 8. 分步实施、候选文件范围与进入条件

| 步骤 | 交付 | 进入下一步的条件 |
|---|---|---|
| P0 复审与启动 | 接受新 ADR；冻结结果模型、文件范围和准备/代理权限；复核最新 main | 适用 blocker 清楚；不自动修改版本；明确 vendor build.rs 的窄例外 |
| P1 诊断与复现 | 冷/热环境变化夹具、准确结果分类、独立兼容与认证报告 | 缺依赖不会误报不兼容；缓存与相同版本不同字节场景有真实执行证据 |
| P2 工具隔离 | prepare 与固定子命令入口、验证后发布、并发/中断恢复 | 全局工具变化不影响项目选择；篡改/链接/离线缺失失败正确；产物被忽略 |
| P3 变化后自动验证 | 接入现有入口；ACL/SQLite 增量失效；干净候选构建 | 变化一定触发所需重建；未变化不会全量重跑；无产品或认证边界放宽 |
| P4 必要安全维护 | 关闭 Cargo 普通 PR 生成；公告清单、事件报告、代理处理说明 | 安全告警仍启用；适用漏洞阻止受影响接受；来源未知/无修复不能假绿 |
| P5 正式验证与交付 | 本地完整验证、PR/main 结果、范围审查、开发说明与完成记录 | 新旧义务保留、实际 SHA 核对、权限撤销；无人接管部分明确标为未实现 |

P1..P5 构成建议的首个有限实施批次，维持现有工具版本。真正的新版本升级采用
后续事件对应的限定维护补丁；不把所有工具升级集中到一次 PR。若无法实现 P4 的
持续代理接管，只能声明检测/报告与手动触发的代理工作流已交付，不宣称全自动维护。

拟允许新增：scripts/dev-toolchain.sh、scripts/verify-toolchain-maintenance.sh、
scripts/toolchain-maintenance.sh（公告报告与固定流程）、docs/provenance/developer-tools-v1.toml、
docs/development/toolchain-maintenance.md、crates/mengxia-testkit/tests/toolchain_maintenance.rs
及其窄测试支持文件。确切共享模块位置在 P0 固定，避免引入新的 Cargo 依赖。

拟允许修改：scripts/verify-toolchain-candidate.sh、scripts/verify-macos-acl-toolchain.sh、
scripts/check-supply-chain.sh、scripts/verify-ci-fast.sh、scripts/verify-ci-supply.sh、
scripts/verify-repository.sh、scripts/verify-maint-001.sh；.github/workflows/ci.yml、
.github/dependabot.yml；crates/mengxia-platform-fs/build.rs；窄 toolchain/CI/naming/document
测试及相关规范、决策、计划、生命周期记录和 AGENTS.md。

例外候选仅为 third_party/libsqlite3-sys-0.38.2/build.rs 的环境跟踪/重建逻辑。
该文件不在旧维护授权内，新 ADR 须单独列出；不修改 vendored SQLite 源码、bindings、
编译安全选项或 runtime identity。若可通过受控构建命名空间完整满足要求，可不改它。

首版禁止更改：产品运行代码、Cargo manifests/lock、Rust/SQLite/protoc/cargo-deny
版本、deny policy、已认证 provenance 内容、数据库迁移/协议字节、数据目录及权限、
第二 UID 的实际安全测试。新 active provenance 切换能力如需独立配置文件，须在
P0 明确路径、闭合解析和分类规则，本规划不授予自动写摘要/自动接受新工具的权力。

## 9. 验收矩阵（候选义务，尚无 PASS）

接受 ADR 时再分配 canonical TEST ID，以下名称仅为规划用例名。

| 用例组 | 必需证据 |
|---|---|
| 正常 OS 更新 | OS/build 改变、工具未变的安全环境可继续；权限/能力真的不满足则准确拒绝 |
| Xcode 更新 | 标准 Xcode.app 原位更新、版本目录切换、相同版本不同字节、Apple 合法分发差异分别检查；本地兼容不自动批准正式认证 |
| 增量正确性 | 热缓存下工具选择/内容/SDK 改变触发 ACL 和 SQLite 所需重建；未变保留增量；中途替换、伪造状态、旧检查器结果不误用 |
| 干净候选 | 证明实际执行 ABI 与编译/运行检查；缓存标记或 cargo 零工作成功不能单独证明新环境 |
| 全局工具隔离 | PATH 中错误 cargo-deny/protoc、Rust override、缺固定版本不静默使用；正常 rustup 默认更新不改变项目版本 |
| 安全安装 | 官方来源/校验失败、篡改缓存、链接逃逸、并发、中断、错误架构、不可写目录拒绝；二次 prepare 幂等 |
| 准备与离线 | inspect 不安装；离线材料齐全可验证；缺依赖/许可证/网络失败不误报 INCOMPATIBLE；源码失败独立报告 |
| 安全处置 | 适用已修复漏洞必须提出候选；无修复/修复不兼容持续阻断受影响接受；非适用有依据；不可用/过期来源不报无漏洞 |
| 公告覆盖 | 产品 Cargo、工具构建依赖、vendored SQLite、Apple/Rust/Actions 公告覆盖逐项说明；未覆盖不宣称安全完成 |
| CI 与权限 | Fast/Native/Supply/second UID/Dependency Review/Merge gate 义务和 SHA 关联保留；未通过或缺失不能聚合成功；无新增 PR 写权限 |
| 恢复与数据 | 仅恢复验证材料或仍安全的工具，不回退数据库或改动用户存储；旧工具不兼容有明确退出路径 |
| 性能与文档 | 测量首次准备、正常热运行、变化后验证耗时/网络次数；不虚构零耗时/全系统兼容；文档不改历史完成证据 |

真实升级证据至少包含一组来源可信的不同 Xcode 安装或升级前后测试环境，以及原位
热缓存更新/身份变化夹具。不能让 fixture 冒充真实 macOS/Xcode 版本支持。无法
获取真实环境时记录 UNVERIFIABLE，不能声称该兼容范围通过。

P5 执行新专项测试与完整本地 developer、docs/fast；PR 和 main 按 ADR-0015 验证
正式原生组件、供应链、第二 UID、汇总及当前 CodeQL 审查。新增证据只运行在其应有
位置，不因工具维护重新增加两套全量 developer/formal。出现新失败才扩大相关诊断。

## 10. 待实施评审闭合的事项

- 环境变化检测对直接 cargo、统一入口及 IDE 的覆盖范围；必须描述边界，不能把
  最佳努力的 Cargo 输入跟踪当作自动监听所有系统更新。IDE 接入按实际调用验证。
- 新工具清单的闭合 schema、版本引用和下载认证实现；不重新引入版本多处硬编码。
- 当前 helper 到结构化结果的迁移方式；旧调用者和测试逐项列出，不能破坏 standalone。
- Apple/Rust/Actions 公告哪些可自动读取、哪些由代理审查，以及最后检查时间/失效
  政策；首版不编造没有实现的公告解析器或全工具扫描能力。
- 常驻维护代理的宿主与凭据若未来开启，需独立选择并验证。按用户偏好减少介入，
  但操作系统管理员步骤、重大权限/许可证/产品支持范围变化仍须明确决策。

这些是实施设计问题，不要求用户现在选 NAS、Linux、最低 OS 或具体新版本。
评审应尽量由实施代理通过实验闭合，不把普通实现细节转成反复的用户确认。

## 11. 官方参考与当前结论的边界

- [Cargo build script change detection](https://doc.rust-lang.org/cargo/reference/build-scripts.html#change-detection)：
  rerun 由可观察输入触发；成功的热 cargo check 不保证重跑 build script。
- [Rustup overrides](https://rust-lang.github.io/rustup/overrides.html)：工具选择有优先级，
  项目文件存在不等于所有调用必然使用该版本。
- [Dependabot security updates](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/secure-your-dependencies/configure-security-updates)：
  ecosystem 的普通版本 PR 限额设为 0 可保留安全更新；alerts/security settings 单独核验。
- [Apple security releases](https://support.apple.com/en-us/100100)：系统/工具公告来源，
  实施时核对当前受影响版本与修复版本，不冻结今天的最新版本为未来目标。
- [RustSec](https://rustsec.org/)：Cargo 生态公告来源，不能代替整个宿主工具链安全审查。

本规划记录来源与拟验证事项，不声称上述漏洞/升级场景已经全部复现，也不声称已经
配置任何自动升级、常驻代理、PR 合并或系统变更。

## 12. 本次规划文档验证

2026-09-12，在上述基线加本次未提交文档差异的工作区执行
`./scripts/verify-repository.sh docs`：PASS。document_traceability、naming、
ci_orchestration、ci_evidence 共 22 个测试通过，git diff --check 通过。
修改仅限本提案、决策日志、实施计划和相应文档版本记录；未修改代码、工具版本或
CI 执行配置，未提交/推送。没有运行完整产品 CI；本结果只证明现有文档门禁通过，
不代表新规划已获接受，也不代表 §9 的拟实施验收已通过。

## 13. 实施审查与本地证据（2026-09-13，尚未正式验收）

ADR-0016 先于代码接受，五项新的 TEST-MAINT3 义务已登记。保留 MAINT-002 的
150 项原始汇总清单；新增五项只在完整 native/supply/second-UID 汇总成功后由
CI 输出 PASS，本地完整 developer 对应 FAST_PASS。专项脚本只输出 COMPONENT_PASS。

已复现并修正：

- 受控真实 Cargo 夹具在未声明环境输入时，热构建改变外部环境后 build.rs 执行次数
  仍为 1；加入指纹 rerun 输入后，变化触发重跑，未变化不重跑。两处原生 build.rs
  均消费该输入。它证明 Cargo 缓存缺口，不冒称真实 Xcode 升级已造成产品损坏。
- 最初跟踪 Xcode 目录/目录符号链接会触发 Cargo 递归扫描；已移除目录跟踪，保留
  具体工具/头文件及外部指纹。没有关闭安全检查或缓存错误 PASS。
- 原子发布使用 link 而非 ln，避免目标已是目录时把候选放入该目录。测试覆盖并发
  正确发布、既有错误文件、目录和符号链接，不覆盖不可信安装。
- 源码改动导致验证前后指纹不同的候选和 Fast 运行已按预期拒绝；这些运行不计 PASS。
  开发中的首轮完整回归已主动停止，最终稳定版本必须另行完整执行。

本机最终冷候选目录为 target/toolchain-evidence/candidate.hs2GW1rH：
COMPATIBILITY PASS / ATTESTATION_MATCH NO，原生 ABI 与 SQLite 身份/硬化测试实际
执行；墙钟 38.84 秒。它不替代完整 task 回归、正式认证或全工具安全审查。
工具 inspect 观察为 7.05 秒、已安装工具再次 prepare 为 0.17 秒；测量期间存在
并行构建负载，只作本机观察，不作为 SLO 或公平的基线性能对比。

供应链当前 advisories/bans/licenses/sources 检查通过。工具公告报告明确 PARTIAL，
Apple/Rust/工具自身依赖等未自动解析来源保持 UNKNOWN；常驻修复代理 NOT_ENABLED。
这些未部署能力不由空事件文件或 cargo-deny 成功冒充完成。

工具版本、Cargo/deny、SQLite 源码/bindings/options、迁移、协议字节及历史认证
manifest 均未改变。工具安装、构建和日志仅在被忽略的 target 路径内；未提交/推送，
未修改 GitHub 权限、关闭现有 PR 或合并 main。Reviewed PR/main 与第二真实 Xcode
环境的证据仍未执行，MAINT-003 保持 IN_PROGRESS，不标记 DONE。

最终完整本地 `scripts/verify-repository.sh developer`：PASS，墙钟 468.19 秒。
149 项原有本地义务加 5 项新增维护义务，共 154 条 FAST_PASS；原始清单中的
TEST-IPC-MACOS-001 真实第二 UID 是第 150 项原有义务，仍由独立 CI 验证，不能
把本地结果说成全部 150 项原有义务的正式通过。工作区 build/check/Clippy/tests、
TASK-001 至 TASK-010 的 developer 专项、MAINT-001/002/003 及一次共享供应链均通过。
最终 docs 门禁的 22 个测试通过；新增工具链专项为 8 个测试，均通过且未忽略。
详细本地日志：target/maint003-developer-final.log、target/maint003-docs-final.log、
target/maint003-candidate-final.log；这些日志不会进入仓库，也不是 reviewed CI 证据。
完整回归后热缓存 Fast 复跑 PASS，墙钟 18.50 秒（target/maint003-fast-warm.log），
用于检查没有因目录递归跟踪而持续拖慢正常开发；不是跨机器性能保证。
