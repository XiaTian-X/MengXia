---
title: "MAINT-002 CI 精简实施规划"
document_role: "Reviewed maintenance plan and completion evidence"
status: "DONE_VERIFIED"
version: "0.1.2"
date: "2026-09-12"
repository_head_reviewed: "8792e6901adfef2ba5cf26d4d5bbb21b5dd5be7a"
---

# MAINT-002 CI 精简实施规划

## 1. 决策与适用范围

已按用户要求复审并完成本次有限维护：减少同一候选提交上的重复验证，保留
全部正式验收责任。ADR-0015 的 S1..S5 已实施并经 PR/main 正式验证（§12），
临时 MAINT_002_CI_ONLY 授权已撤销为 NONE。S6 为 NOT_ENABLED。
TASK-001..TASK-010 和 MAINT-001 的完成状态及历史证据继续有效。

MAINT-002 不是 TASK-011 的产品依赖。维护未完成时，后续任务仍可按现有门禁、
自己的前置决策和独立授权开发；不得因本规划增加新的功能启动阻塞项。

优先完成 S1..S5。S6 的 Draft 优化独立验收；无法证明其合并状态安全时保留现有触发。
路径影响分析、自动推断测试覆盖关系和更换测试框架不进入本次范围。

## 2. 证据与问题分类

| 观察 | 分类 | 处置 |
|---|---|---|
| Formal 和 Developer 分别安装 cargo-deny 并执行大量相同任务映射 | REPO_STALE / 编排效率债务 | 合并公共执行，保留各自独有证据 |
| TASK-007 application_tests、TASK-008 verification_tests、TASK-009 Project/Subject 和 Work/Take 映射重复同一函数 | REPO_STALE / 编排效率债务 | 同一环境下执行一次，明确列出多个义务的映射 |
| 当前 CI 遵守 ADR-0010/0013，正常阻止失败候选合并 | FACT | 不把效率债务归类为现有产品安全缺陷 |
| ADR-0013 要求 code PR 的 Developer/Formal/第二 UID/Dependency Review；候选方案改变 Developer 和供应链归属 | CONFLICT / 候选决策与现行契约不同 | 接受新 ADR 后同步当前规则，不改写历史验收事实 |
| document_traceability 将当前版本、状态和 run ID 写入 Rust 常量，正常完成更新也需改测试 | REPO_STALE / 生命周期维护耦合 | 将可变记录与不变校验规则分离 |
| 新 runner 的冷启动时长、去重收益和更大测试集的运行时间未测量 | UNKNOWN | 测量后报告，不先承诺百分比或耗时 SLO |

已观察的 PR run 34667611801 总耗时 10m27s，Formal 9m50s、Developer 9m30s；
后续 PR run 34668672964 总耗时 12m22s，merged-main run 34669304466 总耗时 11m15s。
这些是少量样本，不证明线性增长、长期分位数或“必然节省一半”。脚本调用点数量
也不等于实际执行次数、独立断言数量或覆盖率。

## 3. 必须维持的功能与安全边界

1. 产品源码、Cargo manifests/lock、依赖和工具版本、deny policy、迁移、协议字节、
   schema、third_party、macOS provenance manifest 的内容不变。
2. 保留 workspace 构建、检查、Clippy、普通测试、现有 doctest/compile-fail、
   CLI/daemon E2E、架构、错误、恢复与安全测试的全部既有义务。
3. 保留 TASK-005 release/ignored scaling、全部规定的 fault/SIGKILL 矩阵、
   TASK-006 恢复测试、TASK-007 100 次 stress 及其余任务正式模式专属检查。
4. attested 和 developer-compatible 证据仍分开。Fast feedback 保留 developer
   构建路径的编译与针对性验证；不能用 attested 路径通过代替 developer 分支覆盖。
5. 真实第二 UID 仍是独立 macos-26 job，使用自己的干净 checkout、attestation
   和构建；其 UID/ACL/socket 拒绝测试不能由普通账户 mock 代替。
6. 保留合并前正式检查、merged-main 代码提交复验、每周 Formal/第二 UID/当前
   advisory 检查、Dependency Review、CodeQL、只读 token、无持久 checkout 凭据、
   full-SHA action pins 和现行 main protection。禁止 pull_request_target。
7. 所有已完成任务的 standalone 命令继续自行执行所需检查，不依赖旧 CI 产物或
   环境中的“已通过”标记。历史 commit 的脚本和证据保持可追溯。
8. 不删除稳定 TEST ID，不降低负向测试和重复次数，不更改产品 cap、错误映射、
   sandbox/Admin/安装/网络等后续能力的启用条件。

## 4. 执行与证据模型

先从当前基线生成旧执行清单：稳定 TEST ID、所需检查、命令、工作目录、
环境、features、profile、filter、ignored/exact、平台、UID、重复次数与准备步骤。
清单覆盖 shell 断言、工具链/proto 检查和 E2E，不仅覆盖 cargo test。

第一版采用显式检查组和映射，避免编写自动解释任意 shell 的执行引擎：

- 完全相同且不依赖先前副作用的检查组，可执行一次供多个 ID 引用。
- 相同测试目标的不同 features/filter/env/profile 不自动视为等价；普通全量
  cargo test 不能自动抵扣 doc、ignored、release、stress 或特权测试。
- 同一 ID 的多个义务全部成功才输出最终 PASS。未知 ID、空映射、缺少测试、
  必需测试被 ignored、零匹配、失败、超时或取消均不能生成该 ID 的 PASS。
- 共享仅限本次调用中显式检查组的执行；不使用可复用结果目录或 PASS 缓存。
  每个直接 Cargo test 调用使用独立临时日志检查非零测试结果，不接受跨 run、
  跨提交、跨构建类别的 PASS。现有子脚本仍按完整退出状态和自身断言验证。
- 输出记录实际 checkout SHA、PR head/base SHA（如适用）、run ID/attempt、
  执行模式、工具链身份、检查映射版本及各检查结果。PR 默认测试合并 ref，
  不将 PR head SHA 和实际 checkout SHA 混为一谈。
- Formal 原生测试结果与供应链结果分开记录；两者未汇总前只称“执行分项通过”，
  不把尚未收到供应链结果的 TEST-SUPPLY-* 或整体 Formal 宣称为 PASS。

公共供应链检查保留固定 cargo-deny 版本、当前 advisory 获取失败即 UNVERIFIABLE、
不可用负向用例以及全部 advisories/bans/licenses/sources。TASK-002 dependency tree、
TASK-004 toolchain、TASK-010 lock/features 等专属断言继续在所属检查中执行。
Dependency Review 检查 PR 依赖变化，不能替代公共供应链检查。

实施顺序先在单进程 repository driver 内合并公共检查，再将其移至同一 workflow 的
独立 job。Merge gate 通过 needs 要求 Formal 分项、供应链、第二 UID、快速反馈及
Dependency Review 成功。不得通过调用者可任意设置的 SKIP_SUPPLY 环境变量生成 PASS。
本地 developer/formal 聚合命令仍完整执行供应链；新的 fast 命令明确只提供快速反馈。

## 5. 拟定文件范围

实施范围为下列清单加 ADR-0015 明确列出的支持文件；越界需求先
更新决策与范围，再修改文件。测试修改仅限验证编排、映射或生命周期的部分。

- `.github/workflows/ci.yml`
- `scripts/verify-repository.sh`、`scripts/check-supply-chain.sh`
- `scripts/verify-task-001.sh` 至 `scripts/verify-task-010.sh` 的十个任务脚本
- `scripts/verify-maint-001.sh`
- 新增 `scripts/verify-maint-002.sh`、`scripts/verify-ci-fast.sh`、
  `scripts/check-ci-merge-gate.sh`
- 新增 `crates/mengxia-testkit/tests/ci_evidence.rs`
- `crates/mengxia-testkit/tests/ci_orchestration.rs`
- `crates/mengxia-testkit/tests/document_traceability.rs`
- `crates/mengxia-testkit/tests/task_010_foundation.rs` 中的 driver 映射测试
- ADR-0015 复核补充：`task_005_foundation.rs` 至 `task_009_foundation.rs` 中
  的 driver 映射测试及 `tests/support/ci_mappings.rs`；产品测试主体不变
- 新增 `docs/spec/task-lifecycle-records.toml`：仅状态、版本、证据引用数据
- 新增 `docs/spec/adr/ADR-0015-ci-evidence-deduplication.md`
- `AGENTS.md`、五份 canonical 文档及本规划：当前决策、启动/完成记录和维护证据

现有第二 UID wrapper、CLI fixture、平台工具链脚本和产品测试主体保持原样。
若旧脚本还有其他测试固定其文本，先定位并列入精确范围，不删除断言绕过门禁。
不为本次维护新增 Cargo 依赖或第三方 action。如需上传证据的新 action，先补充
明确的 SHA/权限/范围评审；首版可使用 job 日志和 GitHub step summary。

## 6. 生命周期数据与未来任务

现有 testkit 没有 TOML 解析依赖。生命周期记录使用无新增依赖的有界、闭合
TOML 子集解析器（section 和带引号的 scalar）；未知/重复字段、非法状态、
缺失或格式错误的证据引用拒绝。记录只能保存 task ID、状态、版本、authority 和
commit/run 引用，不能包含执行命令、任意路径、条件表达式、测试豁免或安全阈值。

校验规则继续要求：DONE 对应 authority NONE；合法开始/完成记录；必需 AC/TEST
集合完整；状态和当前文档一致；历史开始记录不被当前完成状态覆盖。
本地文档测试验证一致性，不把记录中的 PASS 文本当作远端 CI 真正成功的证明；
完成时仍须核验 GitHub 的 commit、run、结果和必需 job 集合。

首版记录覆盖 MAINT-002 生命周期和五份当前文档版本；已完成 TASK 的历史证据
校验不迁移、不放宽。未来 task 在自己的实现阶段扩展闭合 schema 和验收集合，
此后正常开始/完成记录更新只改数据。不能通过新增任意 task 字段自动获取执行权限。

普通完成记录更新可走 docs gate，因为它不改变运行义务。校验器代码、安全规则、
TEST 映射及命令变化始终归类 code。任何新增机器执行映射放在 scripts/ 或测试
目录，不能利用 docs/spec 白名单绕过完整验证。对纯声明数据的允许范围做负向测试。

未来 TASK-011+ 只需登记新增义务和检查组，新增映射默认全量执行。不依靠“叶子
crate”推断跳过前置层。维护不得把 TASK-011 的协议、TASK-012 的平台证明或
TASK-013 的 Admin/持久化义务提前并入本次工作。

## 7. 分步实施与验收

| 步骤 | 交付物 | 进入下一步所需证据 |
|---|---|---|
| S1 基线与决策 | 接受 ADR-0015；记录限定维护授权；旧执行清单和保留义务 | 清单经逐项检查；产品文件基线可比较；现行门禁仍有效 |
| S2 执行去重 | 显式检查组、多 ID 映射；公共供应链在本地聚合内只跑一次 | 原义务到新执行清单无缺失；standalone 语义保留；故意失败检查会阻断所有关联 ID |
| S3 工作流拆分 | Fast feedback；独立供应链；Linux Classify/Dependency Review/Merge gate | 保持现有 PR/main/schedule/dispatch 触发；真实 Linux 分类和聚合测试；完整代码 PR 正式通过 |
| S4 生命周期解耦 | 数据记录及通用状态校验 | 合法开始/完成更新无需改 Rust；非法 authority/证据缺失/规则变更不能绕过；docs gate 通过 |
| S5 对照与交付 | 范围审查、正式 PR 和 merged-main 结果、完成记录 | 所有维护义务通过；功能/安全清单无丢失；仅许可文件变化；撤销维护 authority |
| S6 可选 Draft 调度 | 显式 PR activity types 与 Draft→Ready 门禁 | 下述真实 PR 状态矩阵通过；否则保留 S5 的触发规则并记录延期 |

S3 的快速反馈保留 workspace format/check/Clippy、developer 构建边界回归及明确
列出的快速测试；首版不用路径选择测试。实际冷构建时间可能超过 2–4 分钟，不因
耗时目标删除检查。Formal 继续独立编译和测试 attested 类别。

S6 显式处理 opened/synchronize/reopened/ready_for_review/converted_to_draft。
Draft 的快速反馈有独立成功结果；缺少正式证据时 required Merge gate 实际执行并
返回 failure，不返回 success/skipped/neutral。Ready 后只有当前运行所需证据齐全
才成功。docs-only 使用现行 docs 证据例外，不能将 code Draft 当成 docs。
PR 更新取消旧运行；main 按提交保留运行，不能被后来的 docs 更新取消。
不使用权限升级事件或自动合并处理 Draft。

S6 的真实状态矩阵必须包括：新 Draft、Draft 新提交、同 SHA 转 Ready、Ready 新提交、
Ready 转回 Draft、转换期间取消或失败、重新打开、docs/code 分类切换，以及 fork PR
可用路径。API/平台无法验证的场景记录 UNVERIFIABLE，不能声称已通过。

## 8. 稳定维护验收义务

以下 ID 已随 ADR-0015 纳入 canonical registry；最终结果见 §12，实施中的本地
分项输出本身不代替正式验收。

| ID | 义务 |
|---|---|
| TEST-MAINT2-COVERAGE-001 | 旧/新检查清单覆盖相等，多个 ID 的成功与失败传播，漏映射/零匹配拒绝，模式与环境不同不误复用 |
| TEST-MAINT2-SUPPLY-001 | 公共检查每次聚合执行一次；专属图/lock/feature/toolchain 断言保留；缺失、失败、不可用 advisory 阻断聚合 |
| TEST-MAINT2-CI-001 | 真实执行的事件/分类/job 结果矩阵；failed/cancelled/timed-out/unexpected-skipped 不可通过；旧 SHA 证据不可替代当前候选 |
| TEST-MAINT2-COMPAT-001 | standalone/local developer/formal/docs 命令语义保持，developer 与 attested 边界分别覆盖，产品与冻结输入 diff 为零 |
| TEST-MAINT2-LIFECYCLE-001 | 生命周期数据合法转换、authority 撤销、版本关联、缺失证据和重复记录负向测试；可执行规则仍分类 code |
| TEST-MAINT2-PERF-001 | 记录旧基线和新 PR/main 的总时长、关键路径、runner 分钟、供应链次数及实际执行组数量，不设未测量硬阈值 |
| TEST-MAINT2-DRAFT-001 | 仅 S6 启用时要求真实 Draft/Ready/failure/cancellation 矩阵；延期须明确为 NOT_ENABLED，不能标 PASS |

测试重点是行为和失败传播，不再以 cargo-deny 安装字符串恰好出现两次之类的实现
细节作为安全标准。范围/命令注册校验仍需要精确集合，以防义务静默消失。

## 9. 验证、收尾与停止条件

实施后运行 docs gate、对应的编排/证据/生命周期测试和完整本地 developer 聚合。
正式结果由匹配 provenance 的 macos-26 PR 提供；本机 attested 不匹配按预期拒绝，
不调整 manifest 让它通过。新旧执行计划以基线和候选清单对照；覆盖保留证明加
完整候选 CI，不要求为了统计耗时而重复运行所有历史 task 的递归 standalone。

代码 PR 和 merged-main 都核对完整必需 job 集合与实际 SHA，CodeQL 按现行例外
单独审阅。一次性基线/候选对照完成后，移除临时双跑设置。故意失败只在维护
fixture/临时诊断候选内注入，不修改产品源码或正式分支保护设置。

S5 完成记录明确 S6 是否启用；S6 延期不阻碍 S1..S5 完成或后续产品开发。
维护完成撤销 authority。当前文档的历史版本和已完成任务证据不重写。

出现覆盖缺失、错误 PASS、平台迁移差异或复杂实现需求时，保留尚未替换的旧门禁，
只撤回对应优化。合并后的故障通过普通 reviewed revert 恢复编排，继续保留 main
保护与安全检查，不删除用户数据、不重置历史，不为通过维护验收改产品行为。

收益以实际测量为准。若实现开始要求新测试框架、广泛产品修改或大规模条件裁剪，
停止扩大本次范围，交付已验证的公共去重与快速反馈，继续产品任务。

## 10. 参考证据

- [TASK-010 初次 reviewed PR](https://github.com/XiaTian-X/MengXia/actions/runs/34667611801)
- [TASK-010 后续 PR](https://github.com/XiaTian-X/MengXia/actions/runs/34668672964)
- [TASK-010 merged-main](https://github.com/XiaTian-X/MengXia/actions/runs/34669304466)
- 实施前规则：ADR-0010、ADR-0013；Specification v1.1.45；Plan v0.3.56。
  本次已接受的规则演进见 ADR-0015。
- GitHub 的事件和 skipped-check 行为在实施时复核官方文档与真实 PR；本规划不将
  已知 job 行为推断成所有状态转换均已验证。

## 11. 实施对照清单（S1/S2）

完整旧执行清单以不可变 commit `8792e6901adfef2ba5cf26d4d5bbb21b5dd5be7a`
的下列 Git blobs 为准，而非只用 TEST ID 名称代替测试内容。对应文件保留每条
命令、filter/features/profile、环境赋值、工作目录、条件分支和准备步骤；原 CI
blob 保留 attested、平台和独立 UID 上下文。可用 `git show <blob>` 完整回验。

| 文件（scripts/ 下的脚本省略目录） | 旧 Git blob |
|---|---|
| `.github/workflows/ci.yml` | `15161c00d2145a2dc996fcc9593cbae896ef5d21` |
| `verify-repository.sh` | `5487e876cf28967a494fe154c3722c46a03887ff` |
| `verify-task-001.sh` | `0ee7be240c32741d763118952ddfca19ba096eb9` |
| `verify-task-002.sh` | `e7baec3c9874e291459472fa92aa0fad76916d23` |
| `verify-task-003.sh` | `ab8a46ca214312261c5464b3adf4aa50ef23fe2a` |
| `verify-task-004.sh` | `f87ddd170bc2348fc3a7c294bd299ff83af567c0` |
| `verify-task-005.sh` | `d3361dc655f8d9f1aa8c29499383099188d6b48f` |
| `verify-task-006.sh` | `dd9a29c1c543ce9cd181aa3d156c5d0858923ad2` |
| `verify-task-007.sh` | `10dc9ed59844780dd0352537e1350eaa0a1561e3` |
| `verify-task-008.sh` | `2e94d4091e90c6b5b944cb227538eaf35eddecd9` |
| `verify-task-009.sh` | `bb0c933bf95765a4d36f8cc00c2b6bd5d7410e97` |
| `verify-task-010.sh` | `63877d6ed2a055cc63b154871f1f2c62a8814027` |
| `verify-maint-001.sh` | `cada0466de3fbc4ef9839ce12894b327eb5c1b05` |

逐函数对照：原 95 个 shell 函数中 83 个函数体完全不变；12 个变化仅为八个
执行/结果 wrapper 和四个 supply 函数。wrapper 同时拒绝缺失命令，不允许空映射
生成 PASS；其正常命令参数不变。后者的 TASK-002 图检查、TASK-004 工具链
证明及 TASK-010 lock/features 断言不变，仅公共 deny/不可用负向调用提取到 helper。
所有非函数部分的改变逐项限于参数解析、helper 引入、partial-result 输出、公共
supply 委托和以下四组精确重复映射（原函数体不变）：

| 原调用 | 新执行 | 不变上下文 |
|---|---|---|
| TASK-007 application_tests ×3 | ×1，映射 INGEST/CUSTODY/COMMAND | 同一 mode、features、profile、UID；stress 另行保留 |
| TASK-008 verification_tests ×2 | ×1，映射 VERIFY/CORRUPTION | 原每条测试命令与准备步骤 |
| TASK-009 project_subject_tests ×2 | ×1，映射 PROJECT/SUBJECT | 原每条测试命令与准备步骤 |
| TASK-009 work_take_tests ×2 | ×1，映射 WORK/TAKE | 原每条测试命令与准备步骤 |

不把 workspace 普通测试与后续 filter 测试视为可自动抵扣；本次故意保留这一部分
重复。供应链固定版本/全策略脚本、第二 UID wrapper、产品源码和冻结输入 diff
必须为零。`scripts/ci-baseline-mappings.txt` 提供稳定 ID 精确集合；编排测试验证
集合无缺失/重复，行为测试验证非零 Cargo 结果、失败传播、无缓存和聚合矩阵。
历史执行次数不作为新的压力重复次数要求：只删除上述相同测试程序的冗余启动，
没有减少任何测试函数内部的重复、数据集或规模。

## 12. 完成证据与实测结果（2026-09-12）

S1..S5: DONE；maintenance/product authority: NONE；S6: NOT_ENABLED。

- [正式 PR run 34676854969](https://github.com/XiaTian-X/MengXia/actions/runs/34676854969)
  对应 PR #5 head `9291075a30325d86acf4943b7ef0ab76e0b91cad`；实际测试的是
  merge-ref `e55c3e0fd04680c3679047b0983aacfe222af7b6`，不是把 PR head 冒充 checkout。
- [合并后 main run 34677363307](https://github.com/XiaTian-X/MengXia/actions/runs/34677363307)
  对应 `19e2e613728c2a2c11f6c3dfc185b04e3a625316`。PR head 与合并后的代码树相同：
  `1dde69b3f4173e1f5869f33183f2f6d7411b14b4`。两次 run 的 native/second-UID/supply
  checkout 都与该次 event SHA 一致，Linux 汇总输出完整 REPOSITORY_EVIDENCE PASS。
- [PR CodeQL](https://github.com/XiaTian-X/MengXia/actions/runs/34676853432) 与
  [main CodeQL](https://github.com/XiaTian-X/MengXia/actions/runs/34677363327)
  的 Actions、C/C++、Rust 分析全部成功；完成时查询 main open alerts 为零。
- 初始候选 run `34676434327` 因增加空命令拒绝测试并更新 head 而取消，不能作为
  验收或完整耗时样本；其资源消耗不包含在下表的“单次成功 run”对照中。
- 最终候选本地完整 developer、fast、docs、workspace build/check/Clippy/test
  均通过；新增七项行为测试和历史 TASK-005..TASK-010 映射回归通过。正式
  release/ignored scaling、fault/SIGKILL、100 次 stress 和独立第二 UID 由上述 CI 覆盖。

| 维护验收 | 结果 | 已核验的关键证据 |
|---|---|---|
| TEST-MAINT2-COVERAGE-001 | PASS | §11 逐函数对照；两个汇总日志均与 150 个稳定 ID 清单完全相等；四组显式去重；空/重复/假映射及零匹配拒绝 |
| TEST-MAINT2-SUPPLY-001 | PASS | PR/main 均恰好一次完整全策略检查；当前 advisory 获取及不可用负向保留；图、lock、features、toolchain 专属断言不变 |
| TEST-MAINT2-CI-001 | PASS | PR/main 所有必需 job 成功且 SHA 一致；真实 Linux 自测覆盖事件/分类/失败/取消/跳过/旧 SHA 拒绝；保护设置未变 |
| TEST-MAINT2-COMPAT-001 | PASS | 产品与冻结输入 diff 为零；完整本地 developer、CI attested、独立 UID 均通过；standalone 不继承 supply skip，空命令不能 PASS |
| TEST-MAINT2-LIFECYCLE-001 | PASS | 有界闭合解析器正反例；完成状态、证据引用及文档版本改为数据；DONE 对应 NONE；历史 task 证据保留 |
| TEST-MAINT2-PERF-001 | PASS | 以下官方 job/run 时间及执行日志对照；收益按观察值报告，不设置未测量 SLO |
| TEST-MAINT2-DRAFT-001 | NOT_ENABLED | 保留原事件触发；未启用 Draft 精简，未声称真实 Draft/Ready 转换矩阵通过 |

耗时口径：总时长为 API `updated_at - run_started_at`，包含该次调度等待；job
时长为 `completed_at - started_at`。macOS 合计只累加本 repository-gates workflow
实际执行的 macos-26 jobs，不含独立 CodeQL、Linux jobs 或取消的试验 run，不是账单费用。

| 成功 run | 总时长 | Formal（新为 native 分项） | Developer / fast | macOS job 合计 |
|---|---|---|---|---|
| 旧 PR 34667611801 | 10m27s | 9m50s | 9m30s | 20m39s |
| 旧 PR 34668672964 | 12m22s | 11m51s | 6m58s | 20m22s |
| 新 PR 34676854969 | 9m16s | 8m28s | 1m30s | 13m03s |
| 旧 main 34669304466 | 11m15s | 10m55s | 不适用 | 12m16s |
| 新 main 34677363307 | 9m09s | 8m47s | 不适用 | 12m17s |

新 PR 共享供应链 2m07s、第二 UID 58s；新 main 分别为 2m19s、1m11s。
相对首个旧 PR，单次成功 PR 的 macOS 合计减少约 37%；main 合计基本不变，
不能声称每个场景均减少一半。按一轮 PR+main 合计，32m55s → 25m20s，约减少 23%。
这是小样本观察，runner/队列波动仍存在，不构成长周期性能保证。

旧 PR 日志的 Cargo target/Doc-tests 启动记录为 Formal 556、Developer 537、UID 1；
新 PR 为 native 534、fast 3、UID 1（1094 → 538）；新 main 为 native 534、UID 1。
这是启动日志条数，不是独立测试断言数或覆盖率。旧 PR 完整供应链输出为 11+6 次，
新 PR/main 各 1 次。四个共享测试组在 native 中各执行一次，替代原先的 3/2/2/2 次
调用；除此以外没有自动裁剪 filter/features/profile 或正式矩阵。

收尾只更新本规划、五份 canonical 文档、AGENTS 与声明记录；不再修改 Rust 或
脚本。完成记录通过既有 docs-only 路径校验并受 Merge gate 保护，避免自引用
commit/run 常量导致又一轮代码改动。保留原 main 保护、CodeQL 与每周完整验证。
本次维护到此结束；后续功能开发按原计划和独立 task 授权推进，不追加 CI 优化前置条件。
