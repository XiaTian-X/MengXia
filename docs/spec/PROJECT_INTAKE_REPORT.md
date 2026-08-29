---
title: "梦夏（MengXia）项目接管与仓库基线报告"
status: "TASK_006_IN_PROGRESS"
version: "1.3.27"
date: "2026-08-28"
---

# 项目接管与仓库基线报告

TASK003_CANONICAL_GATE: ACCEPTED
TASK003_SPECIFICATION_VERSION: 1.1.17
TASK003_LIFECYCLE: DONE
TASK003_PROPOSAL: docs/proposals/TASK-003-GATE-PROPOSAL.md

TASK005_CANONICAL_GATE: ACCEPTED
TASK005_SPECIFICATION_VERSION: 1.1.18
TASK005_LIFECYCLE: DONE
TASK005_IMPLEMENTATION_AUTHORITY: NONE
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md

TASK006_CANONICAL_GATE: ACCEPTED
TASK006_SPECIFICATION_VERSION: 1.1.22
TASK006_LIFECYCLE: IN_PROGRESS
TASK006_IMPLEMENTATION_AUTHORITY: TASK_006_ONLY
TASK006_PROPOSAL: docs/proposals/TASK-006-GATE-PROPOSAL.md

本报告只记录只读检查得到的 Current State，不把当前开发机工具或目录当成 Target State 决策。

## Repository facts

| Observation | Evidence | Classification | Impact |
|---|---|---|---|
| Git repository 已初始化，branch 为 `main`，已有文档基线 commit history；TASK-001 bootstrap 属于包含本报告的 repository baseline change | `git status --short --branch`; `git log -1`; reviewed candidate inventory | `FACT / BASELINE CHANGE` | 提交前后均须核对 worktree 与 commit evidence，不得把忽略文件或未暂存文件误报为已提交内容 |
| TASK-001/TASK-002 已完成；workspace 现有 18 个 canonical package，TASK-004 已加入固定 SQLite 3.53.4、精确错误映射、bootstrap-only schema/index/typed-row reopen validator、macOS path/ACL/root/lock authority、pre-mutation clock/UUID first-create orchestration、intent codec/durable-create/post-lock-read、valid-intent empty-staging、staging SQLite bootstrap、ordered publish、closed restart recovery、authorized incomplete/WAL recovery、bounded required-commit WAL classification、23-point/29-case same-OS SIGKILL recovery、bounded connection lifecycle、complete deterministic corruption matrix 与 16×256 WAL-reset stress slices | locked Cargo metadata; repository candidate inventory; TASK-001/TASK-002 evidence; TASK-004 scoped diff and complete local gates; reviewed runner-XIP formal CI run `32695815747` | `FACT / VERIFIED` | TASK-004 `DONE`；正式 supply-chain PASS 来自 reviewed CI attestation；后续 task 不因该完成状态自动获权 |
| TASK-003 的 framed proto3 handshake、server-derived Client identity、受保护 runtime endpoint、CLI/config composition 与 bounded joined lifecycle 已实现；产品 ingest/domain 能力仍不存在 | scoped TASK-003 diff review; `scripts/verify-task-003.sh`; successful CI run `32914222948`; formal job `task-003-second-uid`; `TEST-IPC-MACOS-001: PASS` | `FACT / VERIFIED` | TASK-003 DONE；真实 second-UID evidence 只来自 reviewed formal CI；后续消费者必须保持其 authority boundary |
| TASK-005 exact-scope ports/local-storage/platform implementation 已完成：opaque source/root authority、bounded worker/admission、stream/hash/write、durable no-clobber CAS、orphan/recovery、Location descriptor 与 joined shutdown；本地门禁和 reviewed formal CI 通过 | commits `88e7b3413db5607651f2c842f6d0c1f03d513968`, `f516faafe50707b88f51f25c03be07f917f8943f`; `scripts/verify-task-005.sh formal`; reviewed run `33073580258`; Specification v1.1.19; ADR-0007 | `FACT / VERIFIED` | TASK-005 `DONE`，authority `NONE`；TASK-006 及后续仍未授权 |
| TASK-006 corrected gate v0.2.1 passed review; user accepted v0.2.2 and authorized the exact Asset/domain/command/event/0001 scope after retained baselines passed | accepted proposal hash/current bytes; Specification v1.1.22; ADR-0008; Plan v0.3.32 start record | `DECISION / VERIFIED PRE-START` | TASK-006 `IN_PROGRESS / TASK_006_ONLY`; TASK-007+ remain unauthorized |
| Finder `.DS_Store` 与 Cargo `target/` 存在但被忽略；候选提交清单不包含这些文件 | `git status --ignored`; `git ls-files --cached --others --exclude-standard` | `FACT` | 环境与编译产物不得提交；忽略与强制添加两条路径都由 repository hygiene test 覆盖 |
| 规范 v1.0.1 proposed tree 把 spec/ADR 路径写成 root/`docs/adr`，与实际 `docs/spec` 不同 | document/repository comparison | `SPEC_STALE` | v1.1.0 repository map 已修正为当前 canonical doc path |

## Host and tool facts

| Observation | Evidence | Classification | Impact / gate |
|---|---|---|---|
| 当前检查主机为 arm64 macOS 26.6.2 (build 25G83, Darwin 25.6.0) | `uname`, `sw_vers` | `FACT` | 仅为开发主机事实，不接受 `OQ-001/OQ-002` |
| rustup 1.29.0 已安装 Rust 1.98.0、Cargo 1.98.0、rustfmt 与 Clippy；shell PATH 未被安装器修改 | explicit rustup/toolchain commands | `FACT / VERIFIED` | 使用 pinned 1.98.0 toolchain；TASK-001 创建 `rust-toolchain.toml` 后提供 repository-local resolution |
| PATH 中可见的 `sqlite3` 来自 Android SDK，版本 3.50.6 | command lookup/version | `FACT` | 不是 approved bundled runtime；处于 SQLite 官方 WAL-reset bug 影响版本范围，MUST NOT 被 TASK-004 采用 |
| SQLite 官方 arm64 tools 3.53.4 已安装到用户 MengXia 工具目录；下载 SHA3-256 与官方值 `58d53e...776d` 匹配 | official download + local digest/version/architecture | `FACT / VERIFIED` | 仅用于开发诊断；不替代 application bundled library |
| SQLite 3.53.4 amalgamation 已下载；SHA3-256 `628a44...34e` 与官方值匹配；自编译 CLI 已验证 ADR-0003 compile options | official download + local digest + `PRAGMA compile_options` | `FACT / VERIFIED` | TASK-004 可从已验证源码构建 bundled runtime并加入启动断言 |
| Git 版本为 Apple Git 2.50.1 | version check | `FACT` | 足以进行仓库元数据操作；不是产品依赖决定 |
| 当前 workspace 位于 `/System/Volumes/Data` 的 local APFS | mount/df evidence | `FACT` | 只证明当前 workspace；未来 `MENGXIA_LIBRARY_ROOT` 仍需 exact-path filesystem validation |
| 约 89 GiB 可用空间（检查时） | `df` | `FACT / TRANSIENT` | 不构成性能、最大库或发布容量承诺 |

## Repository/target gaps

| Gap | Classification | Required action |
|---|---|---|
| Rust/MSRV 1.98.0 和 SQLite 3.53.4 工具/源码已接受并完成本机验证 | `DECISION / VERIFIED` | ADR-0003; TASK-001 preserves the pin; TASK-004 still owns application bundling/assertions |
| arm64 macOS foundation 已接受；sandbox backend 未决定 | `DECISION / LATER BLOCKING` | ADR-0004; close OQ-002 before TASK-012 |
| ordinary Client peer UID contract 已接受；Admin mechanism 延后且功能禁用 | `DECISION / FAIL-CLOSED` | ADR-0004; OQ-010 before Admin enablement |
| TASK-002..TASK-005 frame/queue/buffer/concurrency/staging caps 已接受 | `DECISION` | ADR-0005; later caps remain incremental gates |
| secret store、Provider、Rights、retention 未决定 | `UNKNOWN / LATER BLOCKING` | close OQ-004/OQ-005/OQ-008/OQ-009 at their documented gates |
| Repository-local TASK-001 build/lint/test/supply-chain/doc commands 已存在并在当前 arm64 macOS 复核通过 | `FACT / VERIFIED` | 保留这些 gates；后续 task 必须在开始前增加自身稳定 AC/TEST registry 与完成证据 |

## First safe next action

TASK-001/TASK-002/TASK-004/TASK-003/TASK-005 are implemented and verified. Reviewed
runner-XIP CI run `32695815747` proves TASK-004, reviewed real-second-UID run
`32914222948` proves TASK-003, and reviewed `macos-26` run `33073580258` proves the
exact TASK-005 formal candidate. Reviewed TASK-006 proposal v0.2.2, ADR-0008 and the
exact Plan v0.3.32 start record now authorize `TASK_006_ONLY`. The first safe next
action is the accepted TASK-006 implementation order beginning with typed domain
values and immutable migration integration. TASK-007 and later remain unauthorized;
Android SDK/system SQLite remain forbidden.
