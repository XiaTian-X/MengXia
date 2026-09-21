---
title: "macOS 审核准入原生插件：开发计划与首个有界启动草案"
version: "0.1.5"
date: "2026-09-21"
status: "FOUNDATION_DONE_NO_ACTIVE_AUTHORITY"
decision: "ADR-0021"
---

# 审核准入原生插件开发计划

CURRENT_PROJECT_NEXT_ACTION: COMPLETE_BROKER_FOUNDATION

Current scoped start (2026-09-21): BROKER_FOUNDATION v0.1.1 is accepted and
IN_PROGRESS; implementation authority is BROKER_FOUNDATION_ONLY, product authority
is NONE. Exact files/contracts: docs/proposals/BROKER-FOUNDATION-GATE-PROPOSAL.md
§9 and §15; lifecycle evidence is in docs/spec/task-lifecycle-records.toml.
Complete the pure comparison/audit and accounting tests; no IO, launch or durable
lease authority. Earlier draft-only/no-active-authority routing below is historical
and superseded only for this bounded scope; completed-task and strict-profile
evidence remains unchanged. No parent-task completion or production start follows.

当前具体草案（2026-09-21）：BROKER-FOUNDATION-GATE-PROPOSAL.md v0.1.1。
仅定义无执行权的 Run-input read / audit 候选合同；仍待审查与显式接受，
不授权实现，不改变本计划 §9 的已完成基础或后续执行/持久化顺序。
复审修正成员定位为既有 Resource ID + ordinal，并增加独立 plugin_trust fact；
不新增成员 UUID 映射，不以 ProjectTrust、审核或 runtime 合格替代插件信任决定。

REVIEWED_NATIVE_DECISION: ACCEPTED
REVIEWED_NATIVE_PRODUCT_AUTHORITY: NONE
REVIEWED_NATIVE_ADMISSION: EXACT_ARTIFACT_AND_DEPENDENCY_CLOSURE
REVIEWED_NATIVE_MEMORY: MONITORED_NOT_HARD_ENFORCED
REVIEWED_NATIVE_UNKNOWN_PACKAGE: DENY
REVIEWED_NATIVE_SECRET_ACCESS: BROKER_ONLY
REVIEWED_NATIVE_REVOCATION: RECHECK_BEFORE_LAUNCH_AND_AT_BROKER_SINK
REVIEWED_NATIVE_STRICT_EVIDENCE: NOT_INHERITED
REVIEWED_NATIVE_FOUNDATION_GATE: ACCEPTED
REVIEWED_NATIVE_FOUNDATION_LIFECYCLE: DONE
REVIEWED_NATIVE_FOUNDATION_AUTHORITY: NONE

## 1. 决策、范围与不变项

用户已接受 ADR-0021 的路线：macOS 原生主程序、独立原生 worker、逐版本审核准入、
最小权限和持续撤销；明确接受该 profile 未建立硬物理内存上限的剩余 DoS 风险。
不是“安全目标不变但换一个名字”，也不是审核消除了恶意或漏洞。
先支持小规模、能提供源码给审查方、依赖可维护的插件集合，不建设开放自助市场。

§5 纯准入判定与 scoped accounting 已完成并撤销实施授权，证据见 §9；不含生产执行。
具体隔离后端、签名根、发布系统、数值资源
预算、Admin 机制还没有接受；没有生产启动、安装、联网或密钥权限。
本次仅有 §9 结项记录及路线测试修正的正常提交/PR/合并授权，不扩展产品权限。
不启动 VM/Ubuntu，不提权，不安装/运行未知插件，不更改 OS、工具链或 CI tuple。
TASK-012 的严格候选继续 BLOCKED，新 reviewed profile 不继承其全维度 ENFORCED 声明。

保持 TASK-001 至 TASK-011 的功能、协议/manifest、已应用迁移和历史完成证据。
用户明确允许必要时重构已实现代码。以实际接口/安全缺陷为依据调整精确实施范围，
保留外部行为和迁移/兼容性测试，不因“已有实现”强行沿用不合适的结构。
复用包摘要/权限比较、私有协议/有界 session、CAS/资产/恢复；平台 sandbox 仍为占位，
无需推翻一个并不存在的生产沙箱。后续若需改已有架构测试，只放行精确的启动编排层，
不能删除协议层无进程权限、无 Core authority 的保护。

## 2. 完整审核的可执行定义

发布审批与本地权限授权、运行时资格是三份独立证据，缺一不可。

1. **提交与身份**：固定源码提交、许可证、完整依赖/运行时清单、目标平台、能力和
   权限说明。publisher 文本或知名度不是证明；没有足够源代码/构建材料就拒绝首阶段准入。
2. **人工安全审查**：项目指定的人工维护者负责批准；审查敏感路径、解析器、FFI/unsafe、
   进程/网络/文件访问和供应链变更。AI/扫描不能单独签发批准。依赖按风险检查来源、
   漏洞、构建行为和维护状态，不承诺逐行穷尽全部第三方代码。
3. **隔离构建与产物关联**：在独立、无产品凭据的审核环境构建，锁依赖并禁止隐式
   下载可执行内容。记录源码/依赖/工具和构建证明，绑定最终签名/打包后的分发字节
   及实际运行映像清单；哈希只证明字节一致，不能独自证明源码来源。
4. **行为验证**：正常/畸形输入、权限拒绝、输出污染、超时取消、资源增长、子进程与
   清理恢复。恶意测试也必须在适当隔离的审核环境执行，不用用户真实数据或密钥。
5. **发布判定**：有阻断问题不能发布；建立精确包/闭包摘要、权限/profile、平台 tuple、
   源码/构建/测试证据、审查人、签发/到期时间和撤销标识的记录。签名/公证不能替代审查。
6. **持续维护**：每个新 digest 都重新准入；可增量审代码，但检查完整依赖变化和最新
   漏洞情况。运行时禁自更新、远程代码、未审核 dylib/脚本/解释器扩展；代码型模型
   载荷按代码审查。纯数据仍按不可信输入验证，不能靠文件后缀决定信任。

先定义检查表和拒绝理由，不先建设网站。维护者身份、密钥保管/轮换、标准签名格式、
审核隔离环境、审批有效期与漏洞响应时间由发布准入阶段明确；这些未定不阻塞纯函数开发，
但必须阻塞产品安装/执行。关闭审核缺口不能靠用户点一个通用“信任”按钮。

## 3. 安全边界与风险清单

| 属性 | reviewed profile 的要求 | 放行依据 |
|---|---|---|
| 包和实际进程身份 | 精确导入/不可变对象/运行映像及依赖绑定 | 替换、重命名、动态加载与启动竞态测试；不是 pathname hash |
| 文件/网络/IPC | OS 强制拒绝 DB/CAS/HOME/其他 Run/Client/Admin；默认直接网络全拒绝 | 在插件初始化之前生效的真实路径证据；私有 Broker 通道 |
| 秘密与外发 | Broker-only，无原始静态密钥，逐 Asset/目的地/Run 授权 | 本地授权、租约、Rights、SSRF 与审计组合测试 |
| 物理内存与相关宿主成本 | MONITORED_NOT_HARD_ENFORCED，接受明确宿主 DoS 剩余风险 | 增长观测、低并发、输入约束、监测失败拒绝新工作；不承诺无超调 |
| 其他资源 | 每项分别给出预算、OS/host 执行点和证据；不得一并豁免 | CPU/期限、进程/FD、磁盘/tmp/output/log/队列的独立验收 |
| 生命周期 | 独占 worker；限制子进程；取消/退出/回收确认后释放名额 | 异常退出、监控失败、重启恢复、逃逸尝试；未知清理隔离该执行槽 |
| 输出 | 按不可信数据验证，成功前不能当作可信资产/执行指令 | 校验/注册/持久化故障恢复与注入测试 |

审核遗漏、依赖被攻陷和畸形媒体均可能让已审核进程变为恶意进程；仍测试这些攻击。
本决策只降低硬内存可用性保证，不放宽信息泄漏、未经授权写入、密钥和权限边界。
GPU/VideoToolbox 等按能力单独验证；既不承诺全部兼容，也不因为风险就静默删除媒体功能。
若其他不可替代边界无法实现，停在该能力，不再次无限期扩展“完美原生沙箱”研究。

## 4. 阶段、任务归属与退出条件

阶段标签不是新的稳定 TASK ID；父任务不能因纯基础完成就标 DONE。

| 阶段 | 归属与依赖 | 完成边界 |
|---|---|---|
| 准入判定基础 | TASK-012 scoped；只消费已完成 TASK-010/TASK-011 合同 | 纯 typed 判定、负向测试、scoped accounting；无任何可运行 token |
| 执行 profile 资格 | TASK-012 scoped；准入基础与 §0.7 的 BROKER_FOUNDATION | 选一个原生候选，固定 tuple/预算/精确文件；受限自有 fixture 验证真实 custody、隔离、监督；无产品安装 |
| 审核发布与持久准入 | TASK-013；执行资格 + BROKER_FOUNDATION，关闭所需 OQ-010 | 标准信任根/受控构建/审核记录、0003→0004、安装授权/撤销/审计；不伪造 Run |
| 首个真实工作流 | TASK-014/TASK-015；BROKER_PERSISTENCE、执行资格、PLAN_FOUNDATION | 受控媒体能力 + 0005、真实 Run/lease/结果/取消/恢复；先内置，再启用一个已审核样例插件 |
| 真实 Provider | TASK-016 至 TASK-021 中适用的原有门禁 | 凭据、外发、Rights、额度、审计；不得用 CLI 或插件绕开 |
| 发布与持续治理 | TASK-023 + 已启用能力全部前置 | 明确风险披露、可执行撤销/离线策略、系统升级矩阵；不是全 V1 严格隔离 PASS |

原 §0.7 顺序/迁移所有者不变：准入基础是先行的非执行工作；BROKER_FOUNDATION
可以在执行资格前完成，持久 Broker 不成为测试 fixture 的前提。内置实现资格可以
消费新的 reviewed profile，但仍需其完整保留边界。第三方安装/激活要再加审核发布
和持久授权，不会因内置先通过而自动启用。Ubuntu 仍在 macOS 交付后另行启动。

准入持久合同可先验收，但 AC-104/AC-105 的真实启动与 Run 授权终验必须等
TASK-015 的 RUN_INTEGRATION；AC-106 的运行中撤销也一样。阶段标签不是终验
证明，不允许提前创建虚假 Run 以“补齐”证据，也不把后来组合验收变成建库前置。

撤销在启动前和敏感 Broker 操作时重查；已知撤销离线也生效。运行中撤销/过期应
取消并回收 worker、撤销租约；不再发新操作。新撤销无法离线即时获知，只有未过期的
已验证快照能支持离线工作，过期/缺失/检测到时钟或快照回退时拒绝。精确有效期与
响应上限在持久化 gate 选定，不能默认永久有效或强制产品永久联网。

## 5. 首个有界启动草案：准入判定基础

Gate status: ACCEPTED / DONE；仅完成纯准入判定与阶段记录检查，实施 authority 已撤销。
本地启动审查已闭合 §5.4 的 snapshot 绑定/时间/结果合同；不要求先实现完整
运行后端、签名基础设施或恶意插件测试，产品权限仍为 NONE。

### 5.1 输入、输出与算法合同

新增纯内存 reviewed-admission 模块，不修改现有 PermissionDiff/Manifest/proto。
输入是固定大小的 typed facts，不解析任意 JSON，也不构建可直接用于 spawn 的对象。
参数包含候选及审批各自的 manifest/最终分发 artifact/完整 dependency closure/permission/profile/
platform tuple 摘要，review evidence 摘要/策略版本、审批有效区间、撤销状态，以及
调用方提供的当前时间、快照有效区间和时钟/单调 epoch 一致性状态。所有摘要用既有
Sha256Digest 类型；有效期/now 使用 u64 Unix 毫秒，单调 epoch 为独立 u64 计数，
不得互相比较或转换。只做 checked 比较，不读取时钟，不做隐式溢出转换。
未验证、缺失与矛盾事实必须有独立表示，不能用 zero digest 或 false 默认为已验证。

现有 PackageDigest 只标识 canonical manifest 字节，不是整个二进制分发包；不得
复用它表示最终产物摘要。新模块使用分别命名的 manifest/artifact/closure 身份字段，
不改 TASK-010 的摘要算法或历史 fixture。系统库由精确平台/profile 合同约束，
包内及外部非系统可执行依赖则进入精确审核闭包，不要求逐包重新打包整套 macOS。

纯函数只返回 `ReviewEligibilityCandidate` 或确定性的拒绝原因；这个结果不是
InstalledGrant、CapabilityLease、PluginTrustDecision、SandboxEvidence 或可执行权。
调用方能构造的 facts 从来不是经过验签的证明；产品组合必须从受控验证器/持久状态
取得证据，不能接受插件自报的 `verified=true`。没有验证器之前只存在测试输入。

固定判定顺序：

1. 缺失/未验证/未知状态或格式矛盾 → 拒绝；有效区间要求 start < end，end 为排他边界。
2. 已撤销、过期或未生效 → 拒绝；审批和快照都要求 start <= now < end。
3. 时钟/epoch/快照回退或状态未知 → 拒绝。
4. manifest、最终 artifact、完整依赖闭包、权限、profile、tuple 或 review-policy 任何不匹配 → 拒绝。
5. 全部匹配才产生候选；publisher/版本显示文本/ProjectTrust 不参与授权判定。

此函数不计算闭包摘要，不声称掌握 transitive native libraries；真正闭包的计算、
链接/加载约束、长度/内容验证与可执行身份由后续 custody gate 实现。候选和审批的
review evidence 不允许“互相自证”；其真实性由后续受认证的审批记录验证提供。
固定大小输入意味着 O(1) 空间/工作量，无 registry 集合、网络查询、系统时钟读取或
生产资源数值选择。未来序列化/大集合验证另立有界入口，不能悄悄加入此模块。

### 5.2 精确候选文件范围

- `crates/mengxia-plugin-security/src/reviewed_admission.rs`（新增）
- `crates/mengxia-plugin-security/src/lib.rs`（仅模块/类型导出）
- `crates/mengxia-testkit/tests/reviewed_native_admission.rs`（新增）
- `crates/mengxia-testkit/tests/document_traceability.rs`
- `crates/mengxia-testkit/tests/support/lifecycle.rs`
- `docs/spec/task-lifecycle-records.toml`
- `AGENTS.md`、五份 canonical 入口文档、ADR-0021、本计划（仅状态/证据同步）
- 八份已有 proposal/research 路线文档及 ADR-0020（仅当前动作/历史路由同步）

无 Cargo/lock/dependency、protocol/schema/migration、daemon/CLI/platform、spawn/
kill/filesystem/network/keychain/签名/安装/生产审批表的修改。若编译/门禁实际需要
超出这些文件，先列出精确必要差异再更新 gate，不扩大到所有代码。

### 5.3 追踪、验证和完成语义

Feature: FUNC-006, FUNC-007。Requirements: SEC-003, SEC-008, SEC-010, SEC-016,
SEC-017, SEC-022。Acceptance contribution: AC-104, AC-105, AC-106；只提供纯合同
证据，不终验这些产品准入 AC，更不申报 AC-107/AC-108 或 AC-020/AC-021/AC-022 PASS。
Owned planned test: TEST-REVIEWED-CONTRACT-001。本次文档测试仍属于 TEST-DOC-001。

测试覆盖：正常匹配；逐字段 digest/tuple/profile/policy 变更；未知/未验证输入；
审批/快照 start-1/start/end-1/end；整数极值/非法区间；撤销、回退；同作者/同版本
不同产物；多个失败同时存在时拒绝原因稳定。测试不能把 candidate 当产品 authority。

启动时在 lifecycle 记录加入明确 scoped gate、owner、依赖、ID 和 NONE product authority；
第一次实现同时补检查器。完成前验证缺失依赖、缺失/错误证据、范围越权、冒充父任务
DONE、将纯候选作为运行授权均被拒绝。该模块与 accounting 同时通过才记 scoped DONE；
不更改历史 DONE，不要求修改历史版本哈希，不跳过已有测试。

预期命令（未实现的目标不是本轮 PASS）：

```sh
cargo test --locked --offline -p mengxia-plugin-security
cargo test --locked --offline -p mengxia-testkit --test reviewed_native_admission
cargo test --locked --offline -p mengxia-testkit --test document_traceability
cargo test --locked --offline -p mengxia-testkit --test architecture
./scripts/verify-repository.sh developer
cargo fmt --all --check
```

按现有 CI 策略补齐完整 formal/second-UID/supply 证据后才完成该 code scope；
已有稳定 ID 不删减，纯基础完成不会解除任何生产 blocker。下一步先起草
BROKER_FOUNDATION gate，完成该前置后才进入原生执行 profile 的有界实施 gate；
不回到相同硬内存探针，也不自动引入 VM。

### 5.4 启动审查冻结的实现合同

本节细化 §5.1，不增加生产权限。公开输入为固定大小的 ReviewAssessment，含四项
ReviewFact：requirement、approval、revocation、clock。ReviewFact 的 Missing、
Unverified、Invalid、Checked(T) 显式区分未知/未校验/矛盾/调用方已检查的数据；
Checked 不是库验签结果，纯函数不能建立输入来源真实性。

- ReviewIdentity 固定六个分别命名的 Sha256Digest：manifest、artifact、完整 dependency
  closure、permissions、profile、platform tuple。审批和 requirement 必须逐字段一致。
- requirement 指定所需 review policy version 与 review evidence digest；approval
  带同类字段、独立 approval record digest 和有效区间。版本使用 u64 完整值域，
  摘要全零也不是缺失标记；缺失必须用 ReviewFact 表示。摘要真实性/计算仍由后续层负责。
- revocation 指定 approval record digest、Clear/Revoked/Unknown、snapshot epoch 和
  有效区间；即使两个插件的 snapshot 都为 Clear，也必须精确匹配审批记录。
- clock 提供 now_ms、last_observed_ms、minimum_snapshot_epoch；不得读取系统时间。
  时间为 u64 Unix 毫秒，epoch 为独立计数；now 小于 last_observed 或 snapshot epoch
  小于 minimum 都拒绝。持久防回退与可信时钟来源属于后续集成，不由输入自我证明。
- 固定拒绝顺序：四项 facts 按上述顺序检查；审批区间、快照区间、Unknown revocation；
  Revoked；审批未生效/过期；快照未生效/过期；clock rollback；snapshot rollback；
  六个身份字段按上列顺序；policy version；review evidence；snapshot 的 approval 绑定。
  区间 start < end 且 start <= now < end；只比较数值，不加减时间以免边界溢出。
- 结果仅为不可公开构造的 ReviewEligibilityCandidate，保留精确 identity、审批摘要、
  evidence/policy、评估时刻、最早失效时刻和 snapshot epoch 的只读 getter；无序列化、
  grant/lease/launch 转换。最早失效时刻为两个 end 的最小值。它不是可长期缓存的许可，
  启动/敏感 sink 必须重新取得真实证据评估。

阶段记录新增闭合集合 reviewed_native_foundation：owner TASK-012，依赖 TASK-010/
TASK-011 DONE，固定 Feature/Requirement/AC contributor/TEST 组合，gate ACCEPTED，
product_authority NONE、parent_completion NOT_CLAIMED。IN_PROGRESS 只允许上述纯
实现 authority；DONE 必须撤销为 NONE，local_evidence LOCAL_PASS 且有实际 reviewed
PR/main commit/run 引用。只检查格式不能证明 CI 真通过，维护者仍须核验日志/commit。
缺字段/越权/未知阶段/错误 ID/缺依赖/缺证据/父任务冒领均失败；旧 maintenance record
语义不变。先实现检查器再使用 scoped DONE，不依赖尚不存在的产品 Run。

TEST-REVIEWED-CONTRACT-001 映射到
`cargo test --locked --offline -p mengxia-testkit --test reviewed_native_admission`。
现有 TEST-BOOT-002 的 workspace all-targets 命令自动纳入该 target；TEST-DOC-001
验证映射/target 不缺失。新 ID 的正式归属须保留该 target 全部测试实际执行的日志，
不能只借用历史 150-ID/12-ID 汇总行。无需为每个新 case 重启相同测试或改 CI 必需检查。

## 6. 一次性变更影响与剩余决策

| 分类 | 本次处理 | 后续必须闭合 |
|---|---|---|
| CONFLICT：第三方必须全硬限制 vs 审核准入 | ADR-0021 + Specification §0.8 + 新义务，旧 AC 不改写 | profile 逐维度证据；不是强隔离同义词 |
| SPEC_STALE：当前动作仍为 R0-B/VM 选择 | §5 已完成；全入口转为起草 BROKER_FOUNDATION，研究正文明确历史范围 | 不再循环探测同一失败机制 |
| EXPECTED_GAP：没有 production sandbox、准入/撤销服务 | 首个纯合同 gate 与后续职责分开 | 候选 tuple/OS 边界、Admin、发布信任根/撤销/持久化 |
| UNKNOWN：性能、GPU/媒体可用性、具体资源数值 | 不编造预算/性能，不删除媒体目标 | 使用代表性工作负载与原生路径实测 |

本阶段仅完成 §5 基础模块，正式结项证据见 §9；不宣称产品准入完成，安装、第三方包
运行与环境改动仍未授权。下一步起草 BROKER_FOUNDATION gate，不再重复选择已接受方向。

## 7. 本地实现与验证记录（历史检查点） — 2026-09-20

已实现 `mengxia-plugin-security::evaluate_review_eligibility` 和固定大小的输入/拒绝/
candidate 类型，独立导出，不改变 PackageDigest、历史 manifest/protocol 字节、
迁移、依赖或原有权限比较/session 行为。新增 reviewed_native_admission target
覆盖六维身份替换、事实缺失/未验证/无效、审批/撤销绑定、时间区间、撤销、回退、
u64 边界及多错误顺序；candidate 私有字段有 compile-fail 验证。

阶段 ledger 校验已实现；缺依赖、越权、缺本地或正式证据、冒领父任务完成均拒绝。
旧 TASK-010 文档断言只冻结已完成任务的权限撤销，不再冻结全项目永远无实施权限；
新 scope 由独立记录/回归检查。此前混杂的 current-state 页头同步到该 scope。

本地证据基于 HEAD `4fcf3470a6a98ad09feaf12152cee8c69740e467` 加当前未提交 worktree，
不是该提交本身的 CI 证明：

- `cargo test --locked --offline -p mengxia-plugin-security`：3 unit + 1 compile-fail PASS。
- 新准入 target：8/8 PASS；document_traceability：8/8；architecture：4/4；ci_evidence：12/12。
- `./scripts/verify-repository.sh developer`：exit 0，`REPOSITORY developer: PASS`；
  workspace build/check/Clippy/test、既有 task 映射、共享 supply 通过。
  macOS 27 / pinned Rust 1.98；environment fingerprint
  `426c653e1d4d1b82909959202f2bda9bb88e273277dac321e6feb9fef4510841`。
  此完整运行之后只做当前状态/证据文档和对应文档断言修正，单独复验这些变更。
- 修正后的 `./scripts/verify-repository.sh docs`：29/29 PASS（8 traceability、
  4 naming、5 CI orchestration、12 CI evidence）；document_traceability Clippy
  `-D warnings`、`cargo fmt --all --check`、`git diff --check` 均通过。
- 工具安全报告仍为 PARTIAL：上游工具 advisory 适用性 UNKNOWN，不能把 supply PASS
  当成所有开发工具安全审计通过。developer 不包含完整 formal scaling/真实第二 UID，
  本轮未生成 reviewed PR/main、CodeQL 或其他 hosted 证据。

local_evidence 为 LOCAL_PASS；scope 仍为 IN_PROGRESS，PR/main 引用 PENDING，父任务
NOT_CLAIMED。未提交、推送或启用插件。后续先完成该 scope 正式审查/CI，再进入原生
执行 profile 的有界 gate；不要求重做 R0，也不自动启动 VM/Ubuntu。

安全边界复核：Checked 是调用方提供的状态，不是验签结论；返回值只说明输入间关系
通过纯判定。真实来源、签名根、可信时间/持久回退、安装与运行期复核仍待后续实现，
不能把 candidate 作为执行许可，也不能宣称已完成 AC-104/105/106 的产品集成。

## 8. 复审与 PR 验证启动（历史检查点） — 2026-09-20

用户在上节本地完成报告后要求“开始下一步”，现推进已说明的提交/PR CI 阶段，
覆盖纯基础及此前尚未提交的路线/ADR、R0 研究源码。此前“未授权提交”的表述是
上一轮实施边界，本阶段仅扩展正常分支提交、推送与 PR 验证；不自动合并，不启用
产品，不执行新的研究探针，不修改仓库保护规则。

本轮同一 agent 复核纯实现的全部比较/拒绝分支、候选无 authority、阶段记录与
历史测试兼容性，未发现新的阻断缺陷；不冒充独立外部审计。研究脚本仅归档原始
前置来源，shell/C 语法检查不赋予运行资格或新的研究 PASS。
正式 CI 必须实际运行新 target，不能只借用旧稳定 ID 汇总；记录 exact PR head、
tested merge SHA、run 和 CodeQL 结果。合并前保持 scope IN_PROGRESS，main PENDING。
进入实际执行-profile 实施前仍须先满足 BROKER_FOUNDATION，不能跳过 §4 依赖。

结项路径复核补充：测试样本 normalizer 以子串替换 authority 字段，会在实际 ledger
为 DONE 时误改 product_authority。新增 synthetic DONE→progress round-trip 回归先
复现拒绝，再改为 scope 内完整 scalar 行匹配；保留 product NONE 和旧 maintenance
记录。仅测试构造器修正，不改变纯 evaluator 或准入/阶段策略，正式证据必须覆盖修正 head。
同源的 direction 回归也分别消费 IN_PROGRESS 与 DONE，逐项验证负向突变，避免固定
IN_PROGRESS authority 的替换在结项时变成空操作；这两种状态都不授予产品权限。

## 9. 正式结项证据与下一步 — 2026-09-20

用户在 PR 就绪后确认“确定没问题后继续”，授权正常合并、精确 main 验证和有界结项
记录。未绕过保护、修改仓库设置、运行新的 R0/VM 探针或启用产品。

| 证据 | 精确引用与结果 |
|---|---|
| [PR #16](https://github.com/XiaTian-X/MengXia/pull/16) | source `3e08fe2eb6688fb8930d627104d720ecd6f7e5a4`；tested merge `716f4318d00a6864970a508079516f9dfe009815` |
| [PR CI](https://github.com/XiaTian-X/MengXia/actions/runs/35515708386) | 七项任务全部成功；含 Formal、第二 UID、supply、Dependency Review 和 Merge gate |
| 实际 main | `a504def62c9c2283d75391bd94dc159b5b962af8`；与 reviewed source 的 tree 同为 `28fe7fb72d7202849e07fa4f755d5f122d2e2101` |
| [main CI](https://github.com/XiaTian-X/MengXia/actions/runs/35516865035) | Formal、真实第二 UID、supply、Merge gate 成功；证据/expected SHA 全部为上述实际 main；PR-only 任务按既有策略跳过 |
| [PR CodeQL](https://github.com/XiaTian-X/MengXia/actions/runs/35515707457) / [main CodeQL](https://github.com/XiaTian-X/MengXia/actions/runs/35516865043) | actions/c-cpp/rust 分析成功；查询对应 source/main head 无 open alert |

PR/main Merge gate 均保留 167 个唯一既有稳定 ID。新增 TEST-REVIEWED-CONTRACT-001
不冒领旧汇总：两次 Formal 的 `reviewed_native_admission` target 均实际运行 8/8，
无 ignored/filtered；ReviewEligibilityCandidate 的 compile-fail 也通过。
main 日志分别于 14:37:02Z 和 14:41:18Z 记录上述结果；完整 TASK-005 scaling 矩阵
于 14:40:24Z 通过（69.67s）。这些是 hosted 正式证据，不只是本地 developer 结果。

main Rust CodeQL 分析仍列出一个历史 dismissed finding：
[alert #1](https://github.com/XiaTian-X/MengXia/security/code-scanning/1)，
`rust/cleartext-logging`，creative_repository.rs:1830 的 cfg(test) 固定生成 UUID
断言诊断。它在 2026-09-11 已被标记 false positive，该文件本次未改；本轮没有
创建或修改 dismissal。不能把“无 open alert”写成“所有分析零结果”或无漏洞保证。

准入基础 scoped lifecycle 为 DONE，implementation authority 撤销为 NONE，
product authority 仍为 NONE；父 TASK-012 NOT_CLAIMED，严格 Native 候选仍 BLOCKED。
这里只为 AC-104/AC-105/AC-106 提供纯合同证据；不证明输入来源、验签、持久撤销、
安装、真实运行期准入或全部产品 AC。现有功能、wire/manifest、迁移与依赖不变。

结项同步另走有界 PR：仅修改阶段证据、当前路线/版本及 document_traceability 的
两态路线检查。IN_PROGRESS 要求完成准入基础，DONE 要求起草 BROKER_FOUNDATION；
反向状态、未知状态及直接实现/启动命令均被负向测试拒绝。不得为节省验证把测试
变更归类为 docs-only；按既有 code PR/main 门禁复验。该记录修正不改变上述产品
代码 tree 的实施证据；其最终 PR/main 引用记录在结项 PR 中，避免递归证据提交。

下一步仅起草 Specification §0.7 的 BROKER_FOUNDATION gate：明确纯 typed
request/Run/lease/policy/audit facts 的输入、拒绝与有界成本，不执行 IO，不伪造真实
Run，不签发生产授权。精确文件、测试、前置与未完成义务需在草案中冻结，另行接受
后才开始实现；不跳过 Broker 直接进入原生执行，也不重复相同内存探针。
