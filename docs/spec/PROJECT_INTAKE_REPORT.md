---
title: "梦夏（MengXia）项目接管与仓库基线报告"
status: "TASK_001_BOOTSTRAP_VERIFIED"
version: "1.3.0"
date: "2026-08-21"
---

# 项目接管与仓库基线报告

本报告只记录只读检查得到的 Current State，不把当前开发机工具或目录当成 Target State 决策。

## Repository facts

| Observation | Evidence | Classification | Impact |
|---|---|---|---|
| Git repository 已初始化，branch 为 `main`，已有文档基线 commit history；TASK-001 bootstrap 属于包含本报告的 repository baseline change | `git status --short --branch`; `git log -1`; reviewed candidate inventory | `FACT / BASELINE CHANGE` | 提交前后均须核对 worktree 与 commit evidence，不得把忽略文件或未暂存文件误报为已提交内容 |
| TASK-001 Cargo workspace、17 个 canonical package/binary skeleton、Cargo.lock、CI、policy 与 repository verification tests/scripts 已存在 | locked Cargo metadata; repository candidate inventory; TASK-001 verification commands | `FACT / BASELINE-003 SPEC_STALE corrected` | 仅证明 repository bootstrap；不得声称 TASK-002 domain behavior 或任何产品 Feature 已实现 |
| 尚无 schema、migration、domain behavior、IPC、SQLite runtime、CAS 或产品测试 | repository file inventory and empty crate/binary sources | `EXPECTED_GAP` | 后续工作仍须严格遵循 TASK-002 及其依赖顺序和 task-start gate |
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

TASK-001 repository bootstrap is implemented and verified. TASK-002 remains `PENDING` until its stable Feature/Requirement/AC/TEST registry and task-start record satisfy Specification §0.5; this report does not authorize starting it. The Android SDK SQLite remains forbidden, and TASK-004 must later use the bundled path and assertions from ADR-0003.
