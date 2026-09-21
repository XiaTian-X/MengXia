---
title: "BROKER_FOUNDATION：Run 输入读取策略与审计候选合同"
version: "0.1.1"
date: "2026-09-21"
status: "ACCEPTED_SCOPED_DONE"
owner: "TASK-013 scoped foundation"
repository_head_reviewed: "530a3fca9d3def95de51dbd49d23a9fdf326c088"
---

# BROKER_FOUNDATION 有界启动草案

BROKER_FOUNDATION_GATE: ACCEPTED
BROKER_FOUNDATION_LIFECYCLE: DONE
BROKER_FOUNDATION_IMPLEMENTATION_AUTHORITY: NONE
BROKER_FOUNDATION_PRODUCT_AUTHORITY: NONE
BROKER_FOUNDATION_OUTPUT: NON_EXECUTING_READ_AND_AUDIT_CANDIDATES
BROKER_FOUNDATION_PERSISTENCE: NONE
BROKER_FOUNDATION_PARENT_COMPLETION: NOT_CLAIMED

2026-09-21 当前状态：PR #18 和实际 merged-main 均已通过验证，本纯 scope 为
DONE，实施与产品 authority 均为 NONE。精确证据及未覆盖范围以 §17 为准；
下文起草/修复/实施/待验证或无合并授权描述均为历史，不覆盖本结项。
父 TASK-013 不完成；下一步只起草独立执行资格 gate，不授予生产权限。

## 1. 结论和本次交付

本阶段可以先行开发，不依赖未解决的硬内存机制、Admin、真实 Run 表或凭据后端。
v0.1.1 已按 §15 的用户授权接受并启动纯实施，不授予任何产品运行权限。
依据为 Specification §0.7、§0.8、§8.4、§12，ADR-0020、ADR-0021。

选择一个完整但小的操作：**判定一次读取当前 Run 输入成员的请求是否满足提供的
合同事实，返回不可执行的读取候选和审计候选**。不是通用文件 Broker，不返回
文件路径、FD、字节流、真实 CapabilityLease 或可用于启动进程的许可。

这是 TASK-013 的独立 pure scope，不是完成整个 TASK-013。它为后续受限执行
fixture 提供可复用的调用方/输入/租约判定；持久化与真实组合必须继续完成。

启动前置已完成：§10 的验收/测试 ID 已发布到 canonical registry，本版本、数值
边界和 §11 accounting 合同已接受，精确启动记录见 DECISIONS 与 §15。
ledger 已新增 IN_PROGRESS 活动 scope；既有 reviewed-native foundation 保持 DONE / NONE。

## 2. 仓库事实和差异分类

基线是上方精确 main，首次起草前工作树干净；v0.1.1 修复保留既有文档 worktree。
PR #17 的结项修正与 main 验证引用保留
在该 PR；本草案不改写 PR #16 的实现证据，不把文档检查当作 Broker 完成证据。

| 事实 / 分类 | 依据 | 设计处置 |
|---|---|---|
| FACT：准入比较已完成，但 Checked 不认证来源 | plugin-security/src/reviewed_admission.rs | 复用判定，不把 candidate 升格为 grant |
| FACT：Manifest 仅有一种权限 | schemas/plugin/manifest-v1.schema.json：broker.asset.read@1 / run-inputs；最多 1 条 | 本阶段仅处理这一个操作，不改 Manifest/schema/golden bytes |
| FACT：PackageDigest 仅覆盖 canonical manifest | plugin-package/src/manifest.rs | Manifest digest 与最终 artifact/closure/profile/tuple 分开绑定 |
| FACT：现有 PrincipalContext 是普通 Client 的 owner_uid | core-proto/src/lib.rs | 不导入 Core proto，不借 ordinary UID 证明 Plugin/Admin 身份 |
| FACT：ExpectedPluginSession 只有 package/challenge；host 仅有 ping/shutdown | plugin-host/src/session.rs | 不将握手成功或 challenge 当作 Broker caller/run authority；不改私有协议 |
| CONFLICT / MEMBER_IDENTITY：v0.1.0 要求不存在的独立 Member UUID | Specification §8.1；0001_library_assets.sql 的主键 (resource_id, ordinal)；ports 的 member_ordinal | v0.1.1 改为既有 Resource ID + 0..4095 ordinal，不加映射、迁移或 wire 字段 |
| CONFLICT / SEC-004：v0.1.0 缺独立 PluginTrustDecision 输入 | Specification SEC-004 / §12.2 | v0.1.1 在 policy 内显式携带独立 plugin_trust fact；不可从 Project/审核/runtime 推导 |
| EXPECTED_GAP：尚无 Run、持久 Lease、Broker、SecurityAuditEvent 产品实现 | domain、security、events 及当前迁移前缀 | 新类型明确为 facts/candidates；不建假 Run，不写 DomainEvent 冒充安全审计 |
| EXPECTED_GAP：scoped checker 仅支持既有 maintenance/reviewed foundation | testkit/tests/support/lifecycle.rs | 在未来同一次实施中扩展闭合记录和负向测试；不能先写无验证的 DONE |
| SPEC_STALE / ROUTING：旧入口只说“起草”，没有具体草案链接 | 当前路线入口 | 仅补本文件链接；不改变依赖、当前 action 或生产权限 |
| UNKNOWN：Admin、runtime qualification、可信时钟/撤销来源、持久化预算 | OQ-002、OQ-004、OQ-006、OQ-008、OQ-009、OQ-010 的适用部分 | 阻塞相应产品路径，不阻塞纯比较；不在这里编造后端和 SLO |

上述两项冲突由草案修正；其余实现缺失符合已接受的分层计划，不需要推翻已完成实现。没有发现必须修改
现有生产代码才能起草或启动此纯 scope 的冲突。若实施证明存在新冲突，先记录
事实和最小修正范围，不将历史测试删除或把整仓库列入授权。

## 3. 依赖与明确不做的事

必需共同前置：TASK-007、TASK-008、TASK-009、TASK-010、TASK-011 的已接受完成。
本方案还直接复用已完成的 reviewed-native foundation；它作为额外纯代码依赖
记录，不把父 TASK-012 DONE 加为前置，也不改变 Specification §0.7 的公共依赖图。

不做：IO、数据库/迁移、运行期时钟/随机数、线程/异步任务、句柄生成/兑现、租约
签发或消费、签名验证、策略持久存储、Admin、安装/激活、spawn/kill、沙箱、秘密、
网络、Rights 实现、FFmpeg、产品 API/CLI、Ubuntu、R0/VM 实验。
无新 Cargo 依赖、feature、工具链、protocol/schema/fixture 变更。

输出不能实现 From/Into<CapabilityLease>、启动转换、序列化或 public 构造器。
从调用方提供的所有肯定 facts 得到候选也不是认证；这条限制不是仅靠 Rust 私有
字段实现的安全边界，最终受控组合层必须重新认证和决策。

## 4. 类型与数据来源

以下是待接受的精确语义合同；Rust 排列可调整但不能丢字段、扩大集合或引入 IO。
新模块归属 mengxia-plugin-security，复用已有 mengxia-types 和 plugin-package
依赖；不得反向依赖 plugin-host、Core proto、ports、store、domain 或 events。

### 4.1 基础值

- `BrokerKey<K>`：新模块自有 marker 类型的非 nil UUIDv7 包装；`from_bytes([u8;16])`
  调用既有 `Id<K>::from_bytes` 验证，只映射成闭合 `BrokerValueError::InvalidKey`。
  提供只读 bytes getter；不暴露时钟/随机生成函数，不构造 domain Run。
  marker 闭合集合：Library、Project、Run、Instance、Channel、LeaseRecord、Grant、
  AssetRevision、Representation、Resource、Request、Correlation。
  marker 的区分提供类型检查，UUID 不是密码，也不证明对象存在或授权。
- `BrokerDigest`：固定 32 字节，校验语义委托既有 Sha256Digest；全零不是缺失标记。
  包装提供 from_bytes/getter，使 integration target 无需新增 types 直接依赖。
- `BrokerMemberOrdinal`：私有 u32 包装，`new(u32)` 只接受 0..=4095；超界返回
  `BrokerValueError::InvalidMemberOrdinal`，提供 u32 getter；无 unchecked/public 字段
  构造。0 是有效首成员，不是缺失标记；复用现有 ordinal 值域，不生成成员身份。
- revisions/epoch/time/range 都是 u64。revision 必须 > 0；epoch 可以为 0。
  时间为 Unix 毫秒，比较使用 `start <= now < end`，且 start < end；不读取时钟。
- 复用 `ReviewFact<T>` 的 Missing / Unverified / Invalid / Checked(T)。Checked
  仍只表示调用方断言；不新增名字更像已认证身份的类型来掩盖来源缺口。

构造非法基础值返回 `BrokerValueError`，没有半有效 assessment；未来 wire decoder
负责将解析拒绝写入其受控审计入口，本模块不声称审计所有尚未解析的原始输入。

### 4.2 上下文与不可变成员

`BrokerBinding` 固定包含 library_key、owner_uid(u32)、project_key、run_key、
instance_key、channel_key、execution_identity（六个 ReviewIdentity 摘要）。
比较整组字段，不能只比 Run、package digest 或 PID；UID 是 Library owner 的
关联信息，不是 Plugin 身份。生产时 instance/channel 来自 Core 持有的私有通道
和启动记录；PID、challenge、插件自报 ID、普通 Client PrincipalContext 不替代。

`BrokerMember` 按顺序包含 asset_revision_key、representation_key、resource_key、
member_ordinal（BrokerMemberOrdinal）、blob_digest、blob_length。
`ReadTarget` 是此成员和 offset/length；不是 Asset 的可变
head、路径、CAS location 或任意 selector。一个请求只针对一个成员和一个区间。
成员定位精确复用 `(resource_key, member_ordinal)`；不引入独立成员 UUID、随机映射
或以 digest/logical_name 定位的替代路径。后续从现有 ports/wire 接入时验证原 ordinal
并逐值传递；同 Resource 中两个成员即使指向相同 Blob，也必须保持 ordinal 区分。
多成员/大文件由后续上层分次请求，不能在本函数内无限枚举或 materialize。

`ReadAuthorizationKey` = 完整 BrokerBinding + 完整 BrokerMember。
所有作用域 facts 必须精确绑定此 key；即使 blob 字节相同也不允许换 AssetRevision、
Representation、Resource、member_ordinal、Project、Run、实例或通道。Blob 为 Library 级资源，Project 关联必须来自已验证的
Run 输入解析，不宣称 Project 是租户或要求给旧 Asset 表增加 Project 归属。

### 4.3 输入表

主 API 候选：`evaluate_broker_read(&BrokerReadAssessment) -> BrokerReadEvaluation`。
assessment 借用 immutable InspectedPluginPackage，其余均为固定大小的值；不借用
任意 JSON、String、Vec、回调或外部 resolver。输入由宿主准备，不接受 wire 输入。

| 字段 | 固定内容 | 验证 / 来源责任 |
|---|---|---|
| request | request_key、correlation_key、lease_record_key、ReadTarget | 无 actor/run/plugin 字段；都是不可信请求事实，不靠 request_key 授权 |
| caller | ReviewFact<BrokerBinding> | 后续受控私有 Broker 入口认证；纯函数只比较 |
| execution | ReviewFact<ExecutionFact>：binding、run_state、run_revision、有效区间 | RunState 为 Active/Inactive/Cancelled/Unknown；只有 Active 可通过；真实 Run/版本后续查询 |
| input | ReviewFact<RunInputFact>：authorization_key、run_revision、Membership | Membership 为 Present/Absent/Unknown；必须 Present，run_revision 与 execution 相同；后续证明真实输入链/Blob 长度摘要 |
| package | 借用 InspectedPluginPackage | digest 必须等于 binding 的 manifest_digest，且恰有上述既有 read 权限；不能用 publisher/version/capability 名补齐 |
| review | 既有 ReviewAssessment | evaluate_review_eligibility 必须成功；所得 identity 必须与 caller.binding 全等，并在同一 now 评估 |
| policy | ReviewFact<ReadPolicyFact>：authorization_key、policy_revision、grant_key、grant_revision、有效区间、plugin_trust、五项 disposition | plugin_trust 为独立 ReviewFact<BrokerPluginTrustDecision>，必须 Checked(AllowReviewed)；package_policy、installed_grant、project_policy、owner_policy、data_policy 都独立为 Permit/Deny/NeedsApproval/Unknown，必须全 Permit；Run/沙箱/撤销另查 |
| runtime | ReviewFact<ReadRuntimeFact>：binding、qualification_digest、有效区间、disposition | 仅 QualifiedReviewed / Denied / Unknown；前者也只是由后续资格验证器提供的输入，不创造 SandboxEvidence |
| lease | ReviewFact<ReadLeaseFact>，见 §5 | 由后续受控存储查出；不是插件提交的整张 LeaseRecord |
| revocation | ReviewFact<ReadRevocationFact>：authorization_key、grant_key、grant_revision、policy_revision、epoch、有效区间、状态 | 针对当前 grant/policy/binding 的 Clear/Revoked/Unknown；审批撤销仍由 review 单独检查，两者不得相互替代 |
| clock | ReviewFact<BrokerClock>：now_ms、last_observed_ms、minimum_revocation_epoch | 后续可信时间/防回退状态；now < last 拒绝；不把 elapsed duration 与 Unix time 混比 |

ReadPolicyFact 的五项 disposition 是为这一个精确读取从各来源得出的判断，不是
请求字段。installed_grant 必须代表已接受的本地授权；新包不能凭 PermissionDiff
无扩权、审批通过或 package_policy Permit 自动继承旧 grant。policy/ref/current
revision 的真实性仍由持久化阶段保证；同一个攻击者编造一致 facts 不建立权限。

`plugin_trust` 是独立于上述五项的第六项 policy 输入，不是第六个通用 Permit 布尔值。
`BrokerPluginTrustDecision` 为闭合枚举 AllowReviewed / Deny / NeedsApproval /
UnsupportedProfile / Unknown；只有 Checked(AllowReviewed) 可继续。AllowReviewed
表示调用方断言已有本地、精确身份的 reviewed-profile 信任决定，不表示本函数完成
审批或认证；不能由 review eligibility、QualifiedReviewed、package_policy、
installed_grant 或 project_policy 自动生成。旧 SANDBOX_ONLY/TRUSTED_NATIVE 决定
在本合同中为 UnsupportedProfile，不能重命名为 AllowReviewed；未来 profile 扩展另行接受。

plugin_trust 与其余因素共同绑定所属 policy.authorization_key（含六个身份摘要）、
policy_revision 和 policy 有效区间。policy_revision 表示这份完整 policy 快照的版本，
不是仅 package policy 的版本；任一来源决定改变（包括 plugin_trust），后续受控
组装层必须更新该版本，并重新取得匹配的 lease/revocation 事实。policy 有效区间
不得超出任一来源决定的有效区间；缺失/未验证的信任来源必须保留相应 ReviewFact
状态，不用审核通过或 ProjectTrust 替代。持久化 gate 必须证明版本更新、有效期
交集、来源认证和当前撤销的组合；纯函数不验证其来源，也不新增持久化 schema。

`ReadRuntimeFact` 不放行 strict/unknown profile，也不根据“first-party”文本默认
可信。初期 built-in 必须使用另行通过资格验收的 reviewed profile。未来增加 strict
或其他 profile 是显式扩展 gate，不把未支持 profile 映射成 QualifiedReviewed。

本阶段没有 EgressAuthorization、Credential 或 Rights schema。data_policy 仅代表
当前本地 read 的显式 disposition，不代表云处理/外发允许；非读取操作没有此 API。

## 5. 租约事实、时间与范围

ReadLeaseFact 字段：lease_record_key、authorization_key、grant_key、grant_revision、
policy_revision、revocation_epoch、有效区间、state、allowed_offset、allowed_length、
remaining_operations、remaining_bytes。state 为 Active/Revoked/Consumed/Unknown。

它是 **lookup 后的比较模型**，不是随机 bearer token，没有兑换入口，不能拿 UUIDv7
做不可猜测 capability。未来 Broker 自己查找并认证 lease，且必须使用完整 caller
binding，即便攻击者知道或偷到 opaque handle 也不能跨 Run/实例/通道使用。

必须满足：

1. state 为 Active；request.lease_record_key 与记录精确相同。
2. lease/input/policy/revocation 的 authorization_key 均与 caller + request.member
   相同；execution/runtime binding 相同；input.run_revision == execution.run_revision。
3. lease 和 revocation 的 grant_key/revision、policy_revision 与当前 policy 相同；
   lease.revocation_epoch == revocation.epoch；epoch >= clock.minimum_revocation_epoch。
   新 epoch 即使为 Clear 也不自动延长旧租约，由后续层重新取得当前授权。
   policy_revision 同时覆盖 plugin_trust；信任决定改变后，即使 grant revision 未变，
   旧 policy revision 的 lease/revocation 也必须拒绝，不能继续沿用旧允许候选。
4. review.clock.now_ms == clock.now_ms，review candidate identity 全等；审批与 grant
   两种撤销均为当前有效；ReviewAssessment 自身的回退检查不省略。
5. execution/policy/runtime/lease/revocation 五个有效区间均合法且包含 now；
   clock.now_ms >= last_observed_ms。过期、未生效、缺失、未知、回退均拒绝。
6. 请求 length 在 1..=READ_BYTES_PER_REQUEST_MAX；offset <= blob_length 且
   length <= blob_length - offset。空 Blob 无合法非空读取，此阶段无零字节 read。
7. lease.allowed_length > 0；allowed_offset <= blob_length，且
   allowed_length <= blob_length - allowed_offset。request.offset >= allowed_offset，
   delta = request.offset - allowed_offset，delta <= allowed_length，
   request.length <= allowed_length - delta。全部先比较后相减，禁止 unchecked 加法。
8. remaining_operations >= 1 且 remaining_bytes >= request.length。

纯函数不递减预算，不标记消费，不保证单次使用或并发安全。相同有效 assessment
可重复返回相同候选，这不是第二次获得许可。持久阶段必须在同一 authority scope
下原子校验/预留/消费并执行；两个并发候选最多按实际预算兑现，不得先读后记账。
lease TTL、累计 IO/并发/速率及 crash 退还规则须在持久 gate 冻结；这里不拿有限
u64 值域冒充已接受生产配额。候选只保存本次 bytes 上界，不给预算“修改建议”。

## 6. 判定结果和确定性拒绝

`BrokerReadEvaluation` = decision + 一个 `BrokerReadAuditCandidate`。
decision 是 `Eligible(BrokerReadCandidate)` 或 `Denied(BrokerReadDenial)`；本函数
不返回产品 ALLOW/ASK。NeedsApproval 映射为拒绝且不给调用者新授权；真正 Admin
审批 UI/会话在后续拥有自己的 gate。

冻结优先级（多故障时必须唯一）：

1. facts 按 caller、execution、input、policy、policy.plugin_trust、runtime、lease、
   revocation、clock 顺序检查 Missing/Unverified/Invalid；命中则
   `FactUnavailable(kind,state)`。八项顶层 fact 加一项嵌套 fact；只有 policy 为
   Checked 时才访问 plugin_trust，其 kind 固定为 PluginTrust，不 unwrap 缺失 policy。
2. revisions 按 execution.run_revision、input.run_revision、policy.policy_revision、
   policy.grant_revision、lease.grant_revision、lease.policy_revision、
   revocation.grant_revision、revocation.policy_revision 顺序检查零；有效区间按
   execution、policy、runtime、lease、revocation 顺序检查；然后非法 lease 区间、
   非法 request 区间。分别判 `InvalidRevision(kind)` / `InvalidValidity(kind)` / `InvalidLeaseRange` /
   `InvalidRequestRange`；超单次上限优先于 request 的 Blob/range 检查，为 `ReadLimitExceeded`。
3. 全绑定比较按 execution、input、policy、runtime、lease、revocation；每个按
   BrokerBinding 声明字段顺序，再按 BrokerMember 声明字段顺序；区分
   `BindingMismatch(source,field)`。然后检查 run_revision、lease record、grant key、
   grant revision、policy revision 的精确关系；分别返回固定枚举原因，不带值。
4. 时间顺序：clock rollback；execution、policy、runtime、lease、revocation 的
   NotYetValid/Expired；revocation epoch floor；lease epoch mismatch。
5. execution 必须 Active；input 必须 Present；lease 必须 Active；revocation 必须
   Clear。分别返回 `RunNotActive` / `NotRunInput` / `LeaseNotActive` / `RevocationDenied`。
6. package 的 digest、read 权限；然后 review 的 clock 对齐、既有 review 判定、
   identity 对齐。review.clock 为 Checked 且 now 不同时先拒绝；其他状态交给既有
   evaluator 按其冻结顺序判定，不 unwrap 缺失时钟。拒绝为
   `ManifestMismatch` / `ReadNotRequested` / `ReviewClockMismatch` /
   `ReviewRejected(ReviewDenial)` / `ReviewIdentityMismatch`。
7. runtime disposition 必须 QualifiedReviewed；然后 plugin_trust 必须 AllowReviewed；
   再检查 policy 五项按表中顺序均 Permit。拒绝依次为 `RuntimeNotQualified` /
   `PluginTrustDenied(decision)` / `PolicyDenied(factor,disposition)`；其中 decision
   是上述闭合信任枚举。审核通过或 Project Permit 不跳过 PluginTrustDenied。
8. lease 区间包含 request；remaining operations；remaining bytes；拒绝为
   `LeaseRangeDenied` / `OperationBudgetExhausted` / `ByteBudgetExhausted`。
9. 才构造候选，valid_until 为五个事实 end 与 review candidate.valid_until 的最小值。

基础字段 key/digest/member_ordinal 来自 validated typed value；无非法 enum discriminant 的安全
构造途径。拒绝不要求先查数据库，也不会因缺事实 panic；未来 wire unknown 必须
在 decoder 拒绝，不能用默认 Permit/Active/Clear。

BrokerReadCandidate 私有字段固定为 request/correlation/lease record key、完整
authorization_key、run/policy/grant revision、grant key、revocation epoch、runtime
qualification digest、review approval/evidence/policy/snapshot 标识、offset、length、
plugin_trust_decision（只可能为 AllowReviewed）、evaluated_at、valid_until。
成员始终保留完整 resource_key/member_ordinal。只读 getter；不缓存成权限、不带文件路径/句柄/实际字节。
同一调用输出不分配、不 await；任何后续 effect 都必须重新取得认证事实。

## 7. 审计候选，不冒充持久审计

审计合同留在 security 模块，不能塞进 DomainEvent 或普通 domain mutation 流。
每次已构造 assessment 的评估产出一个固定大小 audit candidate，包含：

- schema_version 固定 1，action 固定 ReadRunInput；decision、固定 reason。
- request/correlation key；request 的完整目标（含 resource_key/member_ordinal）和租约
  记录 key 标成 Requested，非已认证事实。不生成审计用的另一套成员 ID。
- caller 为 Checked 时记录其 binding 并标注 CallerAsserted；否则 actor 为 Unavailable，
  不从 lease、input、request 或审核人回填 actor。即使绑定不一致也保留 caller 本身。
- clock 为 Checked 且无 rollback 时记录 evaluated_at；否则 Unknown，不能写 epoch 0
  冒充“已知时间”。其他不可信/畸形事实不补齐正常审计元数据。
- 当前 policy/grant/run revision 与 runtime/review 标识只在对应事实通过格式检查时
  作为 CallerAsserted 元数据；字段无来源就显式 None，不编造版本或成功依据。
- plugin_trust 元数据为 Option<ReviewFact<BrokerPluginTrustDecision>>：仅 policy 为
  Checked 且其 revisions/有效区间格式合法时，保留原 nested fact（含 Missing/
  Unverified/Invalid），否则 None；Checked 决定标成 CallerAsserted，非已认证信任。
  该提取不依赖 evaluator 是否已走到 policy 阶段；其余绑定/时效拒绝不把字段改成
  AllowReviewed。拒绝原因仍按 §6，元数据不能回填 actor 或充当成功依据。
- delegation 为 NotEstablished；本阶段不支持 delegated authorization，后续组合
  必须补齐真实 delegation chain。不是“空 chain 已证明没有委托”。
- event_id、recorded_at 和持久提交状态为 NotAssigned，不生成假 SecurityAuditEvent。

本记录是内部 typed contract，不是 wire/持久 schema；schema_version=1 不冻结
0004 的字节布局。§12.8 要求的真实 actor、delegation、事件 ID、发生/记录时间、
相关性及持久化必须在集成时补齐；candidate 不满足 SEC-019 的终验。
新模块全部 public 结构的 Debug/Display 对 identifiers/digests/targets 全部 REDACTED，
仅显示固定枚举；含 ReviewIdentity 的结构不得派生会递归打印其摘要的 Debug。
不改变现有 ReviewIdentity 的 Debug 合同；新包装边界自行实现脱敏。
没有自由 reason/message/JSON/provider payload/URL/token 字段。读取 getter 不等于
授权日志导出。后续 audit write/query/export 仍是分离的受控路径。

## 8. 有限边界与操作语义

候选常量，须随草案接受而冻结；不是 benchmark 或发布 SLO：

| 项 | 本阶段边界 | 理由 / 超界 |
|---|---|---|
| READ_BYTES_PER_REQUEST_MAX | 1,048,576 字节 | 只限制一次逻辑读取，拒绝 0/超界；不限制 Blob/Asset 总大小；后续可分块 |
| 每 assessment | 1 request、1 member、1 lease、1 独立 plugin_trust fact + 5 个独立 policy dispositions | 无任意集合/递归/扫描，拒绝默认/未知事实 |
| member_ordinal | u32 输入验证为 0..=4095 | 与现有 ResourceMember/ports/0001 一致；不加 UUID 映射 |
| manifest | 借用已通过 TASK-010 上限的 InspectedPluginPackage；最多 1 read permission | 不重新解析/复制 canonical bytes，不增加 parser/dependency |
| 固定大小 owned 输入 / 输出 | 各不超过 16,384 字节，测试 size_of 上限；不包含借用 package 的既有存储 | 是本模块新结构上限，不把 pointer 大小当整个 manifest 内存 |
| 动态分配 / effect | evaluator 与 audit builder 为 0；无 timer/worker、无 IO | 空间与比较次数由固定字段决定，O(1)；基础值构造亦不读外部状态 |

API-010 对应关系：同步单次纯评估；无分页/订阅/流；无副作用/事务/产品幂等状态。
相同输入输出一致，不自行 retry；输入改变后重新评估是新观测。无异步取消点或
外部等待，因此无需虚假 deadline timer；事实的有效期必须检查。评估不是阻塞
syscall，后续 IO 的期限/取消和错误映射由真正 Broker gate 负责。
BrokerReadDenial 是新内部错误，不修改 ErrorCode、CLI/daemon 或历史 wire enum。

1 MiB 不代替生产 Blob IO/磁盘/内存/监控配额；所有真实资源值仍在使用前按 CFG-003
独立接受。调整本常量应版本化并带边界回归，不能从吞吐测量静默放宽。

## 9. 精确实施文件范围

由 §15 的独立纯 scope 启动授权生效：

1. `crates/mengxia-plugin-security/src/broker_foundation.rs`：新增纯 facts/evaluator/audit。
2. `crates/mengxia-plugin-security/src/lib.rs`：仅新增模块和必要 public 导出。
3. `crates/mengxia-testkit/tests/broker_foundation.rs`：新增整 target；deterministic fixtures
   全部就地构造，不造生产 Run、不调用真实启动/时钟/网络。
4. `crates/mengxia-testkit/tests/support/lifecycle.rs`：只增加闭合 Broker scope。
5. `crates/mengxia-testkit/tests/document_traceability.rs`：新增候选合同/ID/阶段/路线回归；
   既有完成测试只作必要的多 scope 兼容，保留所有历史负向断言。
6. `crates/mengxia-testkit/tests/ci_evidence.rs`：仅在新 ledger section 影响 closed fixtures
   时扩展兼容/负向样本，不削减现有 CI/ID/失败条件。
7. `crates/mengxia-testkit/tests/architecture.rs`：只增加新模块纯边界/无 authority 断言，
   保持当前禁止 IO/Core/host 反向依赖的规则，不增加依赖 allow-list。
8. `docs/spec/task-lifecycle-records.toml`：新闭合记录与版本/证据更新。
9. `AGENTS.md`、`docs/spec/IMPLEMENTATION_SPEC.md`、`docs/spec/DECISIONS.md`、
   `docs/spec/IMPLEMENTATION_REVIEW.md`、`docs/spec/IMPLEMENTATION_PLAN.md`、
   `docs/spec/PROJECT_INTAKE_REPORT.md`、本草案、
   `docs/proposals/REVIEWED-NATIVE-PLUGIN-DEVELOPMENT-PLAN.md`：精确 scope/registry/状态。
10. 下列十份既有路线文档：只更新 current-action marker 和当前 scope 导航，历史证据
    不重写：`docs/spec/adr/ADR-0020-builtin-first-macos-delivery.md`、
    `docs/spec/adr/ADR-0021-reviewed-native-plugin-admission.md`、
    `docs/proposals/TASK-012-GATE-PROPOSAL.md`、
    `docs/proposals/DUAL-EDITION-DEVELOPMENT-PLAN.md`、
    `docs/proposals/TASK-012-MACOS-FEASIBILITY.md`、
    `docs/proposals/MACOS-NATIVE-SUPPORT-DEVELOPMENT-PLAN.md`、
    `docs/proposals/MACOS-NATIVE-R0-RESEARCH-DESIGN.md`、
    `docs/proposals/MACOS-NATIVE-R0B-001.md`、`docs/proposals/MACOS-NATIVE-R0B-002.md`，
    以及 `docs/proposals/MACOS-NATIVE-EXECUTION-BOUNDARY-COMPARISON.md`（共十份）。

文件名单仅授权本纯 scope，不授权产品执行。现有 reviewed_admission.rs、
permission_diff.rs、Manifest、session.rs、proto/descriptor、schema/migration、
Cargo/lock、workflow/scripts、platform/daemon/app/ports/events 均不在实现修改范围。
测试可通过现有 dependency 和新模块 wrapper 构造数据，不需要为了测试直接新增
testkit→types 边；如真实编译证明范围不足，列出原因重新接受最小范围。

## 10. 验收与完整测试矩阵

现有 Feature：FUNC-006、FUNC-007；Requirements：SEC-001、SEC-003、SEC-004、
SEC-005、SEC-006、SEC-008、SEC-010、SEC-012、SEC-016、SEC-017、SEC-019、SEC-021、
SEC-022，CFG-003。只贡献 AC-024、AC-026、AC-028、AC-105、AC-106，不能终验。

接受时确认编号未占用；AC-109、AC-110、AC-111 的唯一定义现已发布在
Specification §19.14，两个 TEST 定义在 §20。本草案仅引用；定义不等于 PASS。

已注册 TEST-BROKER-FOUNDATION-001（整 target：所有以下功能/边界/审计用例）；
TEST-BROKER-ACCOUNTING-001（document_traceability 中独立 scoped regression）。
TEST-DOC-001 保持全局文档检查；架构检查继续现有命令，无须为每一字段创建新 ID。

| 组 | 必须执行的正向、负向与组合用例 |
|---|---|
| 正向 | 最小允许、非零 offset、大 Blob 分块、range 刚好触边；输出逐字段和值域/有效期最小值 |
| caller/key | library、owner、project、run、instance、channel 六项逐个替换；同 package 不同实例、重连新 channel、同 Blob 不同资产链；错误 run revision；无信任传递 |
| member compatibility | ordinal 0/4095 成功构造，4096/u32::MAX 拒绝；getter 原值往返；同 Resource、同 Blob 但不同 ordinal 在 request/input/policy/lease/revocation 任一方替换均拒绝，其他链字段不变；候选/audit 保留原 Resource ID + ordinal；不造 Member UUID |
| execution identity | 六个 digest 在 caller/execution/input/policy/runtime/lease/revocation 任一事实上逐个替换；Manifest 仍相同但 artifact/closure 不同也拒绝 |
| facts | 八项顶层和 policy.plugin_trust 各 Missing/Unverified/Invalid；非法 revision/interval；基础 key 非 UUIDv7/nil/wrong variant；unknown 状态不默认成功 |
| independent policy | 五因素逐项 Deny/NeedsApproval/Unknown；全部其余为 Permit 仍拒绝；Manifest 无 read；审核通过但 grant 缺失、旧 revision、新 digest 不继承 |
| plugin trust | Checked 的五种信任决定 × Project 四种 disposition 全部 20 组合，仅 AllowReviewed + Permit 可继续；其余事实全满足仍不能绕过 Deny/NeedsApproval/UnsupportedProfile/Unknown；嵌套 fact 三种不可用状态 × Project 四种 disposition 共 12 组合全部拒绝；审核通过/runtime 合格不补齐信任；policy 六摘要绑定突变、信任变更导致 policy revision 更新而旧 lease/revocation 未更新均拒绝 |
| review/runtime | 复用 review 的 missing/revoked/expired/rollback/六 digest/policy/evidence 矩阵；两个 now 不等；未资格 runtime；review-only 不产生读权限 |
| lease/revocation | 猜到/偷到 lease record key但 caller 变更；Active/Revoked/Consumed/Unknown；错误 grant/policy/epoch；epoch 前移/回退；Clear 不续旧租约 |
| time | 每个区间 start-1/start/end-1/end、空/反向；now/last rollback；零和 u64::MAX；复用 review 的时间/epoch 不混同 |
| byte bounds | 0、1、上限、上限+1；Blob 长度 0/u64::MAX；offset==EOF、offset>EOF；减法前比较；lease start/end、剩余 bytes/ops 0/刚够/差1；overflow 不 panic |
| precedence | 每一相邻拒绝阶段同时触发，断言先者；同阶段字段按冻结序列；全失败输入返回首个事实错误；相同输入确定性 |
| audit | 所有 denial variants / Eligible 都对应一个候选；unknown actor/time 显式无来源；不能从 lease 冒充 caller；plugin_trust 保留原事实状态/决定且符合 None 条件（包括提前拒绝）；不补 AllowReviewed；成员 ordinal 不丢失；正常/错误/Debug 无自由文本、token、URL、path；固定大小 |
| non-authority | compile-fail 私有候选构造；无 IO/clock/random/thread/包导入副作用；重复评估不变更 lease；旧结果不能兑现/转换成 host session 或真实租约 |
| accounting | 缺字段/重复/未知 section、依赖未完成、错 ID/权限/父任务冒领/缺本地或 PR/main 证据均失败；IN_PROGRESS/DONE 两态负向突变都实际发生 |

实施验证命令（实际结果见 §15，不凭命令存在宣称 PASS）：

```sh
cargo test --locked --offline -p mengxia-plugin-security
cargo test --locked --offline -p mengxia-testkit --test broker_foundation
cargo test --locked --offline -p mengxia-testkit --test document_traceability
cargo test --locked --offline -p mengxia-testkit --test architecture
cargo test --locked --offline -p mengxia-testkit --test ci_evidence
./scripts/verify-repository.sh developer
cargo fmt --all --check
git diff --check
```

新增整 target 后将由 TEST-BOOT-002 的 workspace all-targets/all-features 纳入 Formal；
候选稳定映射为 TEST-BROKER-FOUNDATION-001 → 上述 broker_foundation 整 target，
TEST-BROKER-ACCOUNTING-001 → document_traceability 中新增的
`broker_foundation_accounting_requires_dependencies_and_its_own_evidence` 精确 test name。
接受前把映射发布到 canonical registry，首次实现该函数；不得仅声称旧 167-ID 汇总覆盖
新义务：保留新 target 和 accounting 函数全部实际执行、零忽略、非零用例的日志。
不修改既有 ID 列表、脚本/工作流或分类；完整 PR/main Formal、第二 UID、supply、
适用 Dependency Review/CodeQL 和 Merge gate 按 ADR-0015 等现行规则执行。

## 11. Scoped lifecycle 与未来推进

本次在现有 4096 字节 ASCII scalar ledger 内新增闭合 `[broker_foundation]`，
不得把 parser 改成忽略未知字段。闭合字段：id、owner、status、authority、
product_authority、gate、task007、task008、task009、task010、task011、
reviewed_foundation、features、requirements_policy、requirements_evidence、acceptance、acceptance_contribution、
test、accounting_test、parent_completion、local_evidence、pr_head、pr_run、main_head、main_run。

- id BROKER_FOUNDATION、owner TASK-013、gate ACCEPTED、product_authority NONE、parent_completion NOT_CLAIMED。
- task 前置全 DONE；reviewed_foundation 必须对应同 ledger 的 DONE/NONE 实证，不只
  比一个自报字符串；task007/008/009/010/011 要核对既有 canonical 完成记录，不能
  在此复制 DONE 就视作满足。不得修改旧完成证据。
- 候选 scalar 精确值：features = FUNC-006.FUNC-007；
  requirements_policy = SEC-001.SEC-003.SEC-004.SEC-005.SEC-006.SEC-008；
  requirements_evidence = SEC-010.SEC-012.SEC-016.SEC-017.SEC-019.SEC-021.SEC-022.CFG-003；
  acceptance = AC-109.AC-110.AC-111；acceptance_contribution = AC-024.AC-026.AC-028.AC-105.AC-106；
  test = TEST-BROKER-FOUNDATION-001；accounting_test = TEST-BROKER-ACCOUNTING-001。
  每个值均不超过 64 ASCII 字节；仅在候选 ID 确实被占用时于接受前更新精确值，
  不放宽全局 parser limit 或省略义务。新 section 加入后总长必须仍 <= 4096。
- IN_PROGRESS 仅 `BROKER_FOUNDATION_ONLY`；DONE 必须 NONE、LOCAL_PASS 和实际
  reviewed PR/main 精确 SHA/run。格式检查不替代维护者核查真实 CI。
- 初次实施完成 validator/record/regression；之后才可声明或消费 DONE。旧 maintenance
  和 reviewed foundation 的闭合字段、证据与负向测试保持有效。
- 新 scope 尚不存在时，保留当前 DRAFT_BROKER_FOUNDATION_GATE；在接受并启动后，
  route checker 优先选择当前活动 Broker scope 的 COMPLETE_BROKER_FOUNDATION；
  它完成后选择 DRAFT_REVIEWED_EXECUTION_PROFILE_GATE。该最后动作只起草资格方案，
  绝不等同生产启动。两种生命周期及没有新 scope 的旧 ledger 都需正/负测试；
  一次实现支持开始和结束，不能完成时再发现硬编码 IN_PROGRESS。

## 12. 分步执行与后续无冲突衔接

1. 接受前：核对本方案、精确 types/错误顺序/预算、文件名单和待发布 ID；先记录
   canonical supplement 与 scoped start。未解决问题不得写成 PASS/NONE。
2. 实施一：先新增闭合 accounting 与双态负向样本，暂不填 DONE；同时保留历史测试。
3. 实施二：值类型、固定 facts 和 read evaluator；reuse review 原实现不改其语义。
4. 实施三：audit candidate、redaction、compile-fail 和完整矩阵；同一次 scope 完成，
   不分拆成缺测试的可合并授权模块。
5. 本地与 hosted 验证：全量现行 gate；检查新 target 实际执行及精确 PR/main identity。
6. 结项：独立 scope DONE/NONE，父任务不完成；下一步起草原生执行 profile 资格 gate。

后续必须保留的边界：

- 执行资格：可用有标注的纯 facts 测 fixture，不要求真实 Run/lease DB；必须另外
  证明 custody、OS 文件/网络/IPC、实例/通道绑定和清理，不能凭本候选启动未知插件。
- BROKER_PERSISTENCE：仍按 0003→0004；真实审批/本地授权/撤销与 audit 使用独立
  信任来源，最终 sink 重新判定并原子预留预算；schema gate 预先设计 0005 衔接。
  必须证明 PluginTrustDecision 独立来源及其变更推进完整 policy revision/撤销、
  policy 有效期不超出来源交集；不能由 ProjectTrust/审核结果生成插件信任决定。
- RUN_INTEGRATION：0005 建立真实 Run、输入链、实例/通道、lease、audit 及 IO 生命周期。
  此时才有有权使用资源的类型；实现 TOCTOU、防重放、并发消费、崩溃恢复与最终
  descriptor-bound bytes 证明。对失效输入/库故障/撤销 fail closed，不能只查一次。
  输入成员沿用真实 resource_id/ordinal，不新增成员身份表；重验同 Blob 不同 ordinal
  的输入/租约隔离，不能仅用内容摘要连接授权与 IO。
- Secret/Network/Rights：读取允许永不等于上传、解密秘密或删除允许；新增 operation
  必须有独立语义 schema 和对应完整安全 gate，不复用 arbitrary URL/body/path 入口。

## 13. v0.1.0 起草审查与验证记录（历史）

本草案的类型/关系已与当前 package permission、review evaluator、host session、
Core principal、迁移顺序和 lifecycle parser 对照；这是同一 agent 的设计复核，
不是独立外部安全审计或实现证明。仍待显式接受，尚无 Broker 实现/运行期 PASS。
本轮只起草和同步入口；不提交 Git、不运行研究/第三方程序、不变更环境。
本轮实际验证（基于上述 HEAD 加文档 worktree，不是新 commit/hosted 证据）：

- `./scripts/verify-repository.sh docs`：29/29 PASS（8 traceability、4 naming、
  5 CI orchestration、12 CI evidence）。现有全局检查不会自动证明新草案每一条
  语义或未实现的新测试；新模块/两项新稳定映射的实证仍为未实施。
- `cargo test --locked --offline -p mengxia-testkit --test architecture --test reviewed_native_admission`：
  4 + 8 PASS；既有边界/准入比较兼容，不是 Broker 测试。
- `cargo fmt --all --check`、`git diff --check`：PASS；生产和测试源码 diff 为空。
- 只读结构核对：§9 共 25 个不重复精确文件，其中仅模块与整 target 为未来新增；
  其他路径均存在。五个候选 ID 未占用；候选 scalar 最长 63 ASCII 字节；
  仅在内存构造的合并 ledger 容量样本为 1,879 字节，未写入真实 ledger，也不证明
  尚未实现的 Broker checker 可接受它。未来仍须执行 §11 的完整负向验证。

当时结论（后续复审发现的两项合同问题以 §14 为准）：未发现需要改变已实现生产代码或已接受安全方向的前置阻断；可以进入草案
审查/显式接受，不可据此跳到生产运行。本轮没有新的正式 CI、独立外部审计或
Broker 产品验收结论；未来 §10 的未实现命令不计作 PASS。

## 14. v0.1.1 修复与复审 — 2026-09-21

本轮按用户“开始修复”处理上一轮两项 P1；gate 仍为 DRAFT、实施与产品 authority
均为 NONE。修复限于本草案与现有导航/审查/版本记录，没有新活动 ledger section。

| 问题 | 本次处置 | 复审依据 / 未实施边界 |
|---|---|---|
| MEMBER_IDENTITY | 移除独立 Member UUID；§4 使用私有有界 BrokerMemberOrdinal，§5/§6/§7 保留完整 Resource ID + ordinal 绑定和审计，§10 要求同 Blob 不同 ordinal 的替换拒绝 | 对照既有 0001 主键、ports/应用及 wire selector；0 和 4095 有效，4096 和 u32::MAX 拒绝；不引入映射、迁移或协议变更；行为用例待首次实施 |
| SEC-004 | policy 显式增加嵌套 plugin_trust fact；独立状态、精确身份绑定、快照版本/有效期/撤销关系、冻结拒绝顺序和审计元数据均已定义 | §10 要求 20 个已提供决定 × Project 组合和 12 个不可用状态 × Project 组合；审核/runtime/Project 不能替代，持久化必须另证可信来源与变更传播；不是本地授权实现 PASS |

复审结论：这两项草案缺陷已在合同层修正，未发现由本次修复引入的实现依赖或
已实现功能冲突；不需要重构现有生产代码，也不需要改变 ADR-0021 的安全取舍。
独立信任事实借用完整 policy 的既有绑定/有效期/版本，不扩展到 Admin、签名或数据库
实现。文件名单、候选 ID、有限尺寸上限、迁移顺序、最终 IO/预算/撤销门禁保持不变。
这仍是当前 agent 的代码/规范对照复审，不是独立外部审计或运行期资格证明。

本轮验证（同一 main 基线上的文档 worktree，未提交）：

- `./scripts/verify-repository.sh docs`：29/29 PASS。首次检查命中新增数值区间的
  文档语法限制，已改为明确含端点的文字表述后全量重跑；未修改 checker。
- `cargo test --locked --offline -p mengxia-testkit --test architecture --test reviewed_native_admission`：
  4 + 8 PASS，验证保留的架构/准入边界，不计为新 Broker 行为测试。
- `cargo fmt --all --check`、`git diff --check`：PASS。
- 临时只读结构核对：成员类型/绑定、信任独立性/拒绝顺序/矩阵、审计来源及
  DRAFT/NONE 权限标记一致；不是已入库的回归测试，不替代 §10 的后续行为验证。

本轮未修改生产/测试源码、旧协议/迁移、依赖/工具链、CI 或历史完成证据，未运行
新研究/第三方插件，未提交/推送。下一步是显式接受 v0.1.1 并发布 canonical
验收/测试合同与精确启动记录；不能以本节授予 Broker 或原生执行权限。

## 15. 接受、实施与本地验证 — 2026-09-21

用户要求“再次独立审查，没问题后可以继续”。重新对照代码/规范后未发现本纯
scope 的未解决实施阻断，故按该条件授权接受 v0.1.1 并推进本地实施。
这是当前 agent 的重新审查，不是另一个外部审计者或运行期安全资格认证。
canonical 合同已发布至 Specification §19.14/§20；DECISIONS 记录精确 §9 范围。
计数更正：§9 是 26 个路径（含“本草案”），历史 §13 的 25 漏算了自身；未新增
授权路径。实际改动 25 个路径，ci_evidence.rs 保持不变且兼容性验证通过。

当前 scope 为 IN_PROGRESS，implementation authority 为 BROKER_FOUNDATION_ONLY，
product authority 为 NONE；没有父任务 DONE 或原生执行权限。当前 HEAD 仍为
530a3fca9d3def95de51dbd49d23a9fdf326c088，证据只覆盖其上的本地 worktree，未提交。

已实施：

- security/broker_foundation.rs：固定 facts、独立 plugin_trust、Resource ID + ordinal、
  全身份/版本/时间/撤销/字节范围比较；私有 read/audit candidates，无 IO/随机/时钟
  读取、无动态分配、无租约消费或 host 转换。reviewed_admission.rs 未改。
- Audit：Requested 与 CallerAsserted/Unknown 分离，缺 caller 不从 lease 回填；
  policy.plugin_trust 保留原事实状态。所有新载荷结构的 Debug/Display 脱敏。
- Accounting：闭合新 section、验证既有准入基础完整 DONE 实证及五项 canonical
  task 前置；开始/结束/旧 ledger 路由均有正负样本。旧 reviewed scope 的重开测试
  使用不含新依赖 scope 的历史 fixture，不撤销真实已完成状态。
- broker_foundation 整 target、候选/ordinal 私有构造 compile-fail、架构负向检查。
  无 testkit 新依赖；无脚本/工作流/迁移/wire/manifest 变更。

确定性实现细节：§6 的关系错误使用 RunRevisionMismatch、LeaseRecordMismatch、
GrantKeyMismatch(source)、GrantRevisionMismatch(source)、PolicyRevisionMismatch(source)；
同组 source 顺序为 Lease 后 Revocation。租约区间格式使用租约自己声明的 Blob 长度
检查，request 使用自己的长度，然后按 §6 比较完整成员绑定。审核 snapshot 标识为
既有 approval-record digest + snapshot epoch，不创造未提供的 snapshot digest。

BrokerReadDecision 的 Eligible 变体保留固定大小内联候选；对该枚举单独使用有理由
的 clippy::large_enum_variant expect，以满足已接受零分配合同，未放宽全局警告规则。
输入/输出仍分别有 16,384 字节上限的回归断言，不使用堆分配规避警告。

本地证据：

- `cargo test --locked --offline -p mengxia-plugin-security`：3 unit + 3 compile-fail PASS
  （含原有准入候选、新增读取候选与 ordinal 私有构造）。
- `cargo test --locked --offline -p mengxia-testkit --test broker_foundation --test document_traceability --test architecture`：
  15 + 9 + 5 PASS；包含全信任交叉矩阵、逐字段身份/成员替换、边界与多故障顺序。
- scoped all-target/all-feature Clippy（warnings denied）：PASS。
- 完整 `./scripts/verify-repository.sh developer`：PASS（exit 0），包含 workspace
  回归、Clippy、既有 task/维护及供应链门禁；这是 Developer 证据，不是 Formal。
- 最终证据同步后重跑 `./scripts/verify-repository.sh docs`：30/30 PASS
  （9 traceability、4 naming、5 CI orchestration、12 CI evidence）；
  Broker/架构回归 15 + 5 PASS，fmt 与 diff 检查 PASS。ledger 为 LOCAL_PASS，
  本地证据只覆盖当前 worktree，不冒充精确提交 CI。
- PR/main Formal、真实第二 UID 和新 hosted CodeQL：未执行，不用历史 CI 替代。

本地验证已完成；未取得 reviewed PR/main 证据前不消费 scoped DONE。本次用户授权不含
提交、推送或合并；不运行第三方插件，不启用生产 Broker、Admin、凭据或新研究。

## 16. PR 验证阶段 — 2026-09-21

用户在上述本地报告后要求“开始吧”。DECISIONS 的 PR validation authorization
允许提交现有精确范围、推送工作分支并创建普通 PR；仅替代 §15 当时的提交/推送
限制，不授权合并或产品启动。当前保持 IN_PROGRESS / BROKER_FOUNDATION_ONLY，
product authority NONE；PR/main 字段仍 PENDING，不预填未观察到的成功。

本地实现证据仍见 §15。正式证据以实际 PR head、对应测试 checkout 和完整 job
日志为准，必须确认新 broker_foundation 整 target、accounting 精确函数及私有
构造 compile-fail 实际运行，不能只引用旧稳定 ID 汇总。所有适用的 Formal、
真实第二 UID、供应链、Dependency Review、CodeQL 和 Merge gate 均保留。
未取得合并授权与精确 merged-main 实证前，不关闭 scope 或启动执行资格实现。

## 17. 精确合并验收与 scoped 结项 — 2026-09-21

用户明确要求复审后合并。重新审查 exact head 的绑定、独立信任、事实有效期、
双重撤销、范围/预算、确定性拒绝、脱敏候选及双态 accounting，未发现本纯 scope
的阻断问题；本次是当前 agent 的复审，不是独立外部运行期安全审计。
PR #18 已按正常保护规则 squash merge，没有管理员绕过、强推或额外产品能力。

| 证据 | 精确对象与结果 |
|---|---|
| Reviewed source | 37b11896a339158807e050f007aea54fc7600349；PR #18 |
| PR CI | run 35548864113：Formal、第二 UID、供应链、PR feedback、Dependency Review、分类和 Merge gate 全部 SUCCESS |
| PR test checkout | 11a8472b659fc838678c85a601d4b9138608c52d，parents 为 source 与 base 530a3fca9d3def95de51dbd49d23a9fdf326c088；tree 与 source 相同，不冒充实际 main |
| Merged main | 88d1ac06fa4a9bcc4c2877cf1c67dbe68f01fc6c；代码树与 reviewed source 相同 |
| Main CI | run 35550089829：Formal、第二 UID、供应链、分类和 Merge gate 全部 SUCCESS；PR feedback/Dependency Review 按 push 规则 SKIPPED，不计 PASS，其 PR 证据在上方 |
| CodeQL | PR run 35548862624 与 main run 35550089502；Rust、C/C++、Actions 均 SUCCESS，分析记录分别绑定对应 source/main SHA，无分析 error/warning |

两次 Formal 均从日志核对：TEST-BROKER-FOUNDATION-001 对应 broker_foundation
整 target 15 passed / 0 failed / 0 ignored / 0 filtered；security 的三个
compile-fail 全部通过，包含 BrokerReadCandidate、BrokerMemberOrdinal 和保留的
ReviewEligibilityCandidate。TEST-BROKER-ACCOUNTING-001 对应精确函数
broker_foundation_accounting_requires_dependencies_and_its_own_evidence 已运行成功；
document_traceability 整 target 9 passed / 0 failed / 0 ignored / 0 filtered。
不依赖旧稳定 ID 汇总来替代这些实际执行证据；既有正式矩阵保持有效。

CodeQL 的 main Rust 结果仍含一个此前已 dismissed 的 fixture false positive
（alert #1、creative_repository.rs、rust/cleartext-logging）；本轮未重新关闭或
抹除该历史记录，没有新增开放告警。供应链仍保留 TOOL_SECURITY_COVERAGE: PARTIAL
的既有覆盖限制，不将工具依赖审查 UNKNOWN 写成全面安全 PASS。

结项 disposition：AC-109、AC-110、AC-111 与两项新稳定测试的本 scope 义务通过，
ledger 为 DONE / NONE / LOCAL_PASS，填入真实 PR/main SHA/run；此前已完成记录
保持原样。贡献性 AC 不因此终验，父 TASK-013 为 NOT_CLAIMED，strict TASK-012
继续 BLOCKED。当前无实施或产品 authority；下一步只起草 reviewed execution-profile
qualification gate，不启动其实现、持久 Broker、插件、Admin、迁移、Ubuntu 或研究。

结项文档和 ledger 经既有验证器检查，走独立普通文档 PR；该记录不会改变上方
实现验收 SHA。后续文档 PR/main 的验证在对应 PR 中记录，不用自引用提交或无穷
改写历史证据。结项不需要修改现有生产代码、测试、协议、迁移、依赖或 CI。

结项本地验证：`./scripts/verify-repository.sh docs` 的 30 项通过；Broker/架构
回归 15 + 5 通过，`cargo fmt --all --check` 与 `git diff --check` 通过。既有
maintenance/reviewed foundation ledger section 与 main 基线逐段相同，源码及
scripts/workflow/协议/迁移/manifest diff 为空。文档 PR 后续结果不是新产品验收。
