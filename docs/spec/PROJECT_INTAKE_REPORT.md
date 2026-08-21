---
title: "梦夏（MengXia）项目接管与仓库基线报告"
status: "FOUNDATION_TOOLING_VERIFIED_READY_TASK_001"
version: "1.2.2"
date: "2026-08-21"
---

# 项目接管与仓库基线报告

本报告只记录只读检查得到的 Current State，不把当前开发机工具或目录当成 Target State 决策。

## Repository facts

| Observation | Evidence | Classification | Impact |
|---|---|---|---|
| Git repository 已初始化，branch 为 `main`，已有文档基线 commit history | `git branch --show-current`; `git log -1` resolves the reviewed documentation baseline | `FACT / SPEC_STALE corrected` | TASK-001 仍是首个实现/CI bootstrap；不得把文档 commit 误报为代码已初始化 |
| 无 Cargo workspace、源代码、schema、migration、测试或 CI | repository file inventory | `EXPECTED_GAP` | 与 Architecture/Phase 0 一致；不得声称功能已实现 |
| 当前文件仅为 `AGENTS.md`、`docs/spec/*` 与 Finder `.DS_Store` | file inventory | `FACT` | `.DS_Store` 属未跟踪环境文件；TASK-001 应建立忽略规则，不得擅自删除用户文件 |
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
| Rust/MSRV 1.98.0 和 SQLite 3.53.4 工具/源码已接受并完成本机验证 | `DECISION / VERIFIED` | ADR-0003; TASK-001 may start; TASK-004 still owns application bundling/assertions |
| arm64 macOS foundation 已接受；sandbox backend 未决定 | `DECISION / LATER BLOCKING` | ADR-0004; close OQ-002 before TASK-012 |
| ordinary Client peer UID contract 已接受；Admin mechanism 延后且功能禁用 | `DECISION / FAIL-CLOSED` | ADR-0004; OQ-010 before Admin enablement |
| TASK-002..TASK-005 frame/queue/buffer/concurrency/staging caps 已接受 | `DECISION` | ADR-0005; later caps remain incremental gates |
| secret store、Provider、Rights、retention 未决定 | `UNKNOWN / LATER BLOCKING` | close OQ-004/OQ-005/OQ-008/OQ-009 at their documented gates |
| 尚无 repository-local build/lint/test 命令 | `EXPECTED_GAP` | TASK-001 creates the first project verification commands; external toolchain/evidence checks above already pass |

## First safe next action

The approved Rust/SQLite tooling is installed and verified. The first safe implementation task is now `TASK-001`. The Android SDK SQLite remains forbidden; application persistence must later use the bundled path and assertions from ADR-0003.
