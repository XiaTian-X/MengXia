# 开发环境与工具维护

当前仅支持已接受的 arm64 macOS 开发范围。系统补丁或兼容的新 Xcode 无需与 CI
版本完全相同；本地兼容不代表正式认证，也不证明成品最低运行系统。

## 日常命令

- `scripts/dev-toolchain.sh inspect`：只读环境/工具观察，不安装、不宣称 ABI 或安全通过。
- `scripts/dev-toolchain.sh prepare --network`：隔离准备当前固定 cargo-deny。首次联网
  下载官方发行物并验证已评审摘要；不覆盖全局工具，也不升级 Rust/Xcode/系统。
- `scripts/dev-toolchain.sh check`、`build`、`test`：固定、离线、locked 的全工作区操作。
- `scripts/dev-toolchain.sh fast`：已有 Fast 反馈，加工具链专项回归。
- `scripts/dev-toolchain.sh verify`：在新的 target/toolchain-evidence/candidate.* 下
  冷构建原生依赖并执行 SQLite 身份/硬化测试；保留日志，不复用候选 PASS。
- `scripts/verify-repository.sh developer`：完整本地门禁，保留全部产品测试和一次供应链。

系统/Xcode/Rust/检查政策或相关输入变化时，维护入口每次重新观察工具身份并设置
原生构建指纹，触发 ACL/SQLite 重建和下游重链接。未变化时仍可增量构建。
原始 `cargo` 命令仅依赖已声明的外部输入跟踪；OS 更新后必须先使用维护入口。
IDE 未配置调用本入口时同样如此；本次不修改全局 IDE 设置。

首版不会自动安装 rustup、Rust 组件或完整 Xcode。缺失时由代理按项目 pin 准备；
许可证/管理员安装/重启需要用户完成。明确设置的编译覆盖变量会被拒绝，不静默
改变用户配置。RUSTUP_TOOLCHAIN/global default 不影响维护入口的显式项目选择。
完整门禁要求默认 target 布局；不支持通过 CARGO_TARGET_DIR 改写 CLI 二进制位置。

## 失败与恢复

NEEDS_PREPARATION 是缺少材料/离线解析失败，不等同于系统不兼容。UNVERIFIABLE 是
身份/权限/证据无法验证。SOURCE_CHECK_FAILED 表示冷构建或测试失败，需看保留日志
诊断；没有归因证据时不臆测为 ABI 破坏。ATTESTATION_MATCH: NO 不阻断兼容开发。

工具安装通过唯一临时目录和原子 no-clobber 发布，重复/并发运行可复用正确结果。
缓存二进制每次先校验字节再执行。中断的 prepare.* 目录不被当成已安装；再次 prepare
使用新目录。异常遗留目录交由代理确认路径后清理，不能递归清理整个 target。
篡改缓存拒绝执行和覆盖；由代理隔离被篡改的精确文件后重新准备。

## 安全维护：已有自动化与边界

Cargo 普通追新 PR 已关闭，安全更新保留；Actions 必要维护仍有候选。现有每周和
代码供应链检查保留。`scripts/toolchain-maintenance.sh report` 输出逐源覆盖状态；
它不联网核实每条 Apple/Rust/工具公告，UNKNOWN 不是通过。

维护代理收到公告后应核实实际组件/配置，把事件写入机器消费、走 code CI 的
`docs/provenance/toolchain-security-events-v1.tsv`：公告 ID、工具、状态、官方公告、
评审证据链接及日期。AFFECTED/UNKNOWN 阻断供应链验收；RESOLVED/NOT_APPLICABLE
必须经评审并提供证据。无记录不是“无漏洞”，删除/关闭记录必须是受审查的变更。
首版不支持自动安全例外；需要例外时先遵守 SEC-020 的有期限 ADR，再专项实现。

安全修复必须持续跟进，失败的 Dependabot PR 不能直接放弃。代理准备最小升级，
同步当前合同/fixture，执行兼容与完整 CI，保持历史证据。只可恢复仍安全且受宿主
支持的旧工具，不自动回退 OS、数据库或启用已知受影响版本。

常驻修复代理、全工具公告自动解析和自动合并均未启用。当前交付是隔离/兼容检查、
现有自动安全检查、事件阻断与可由代理执行的维护流程，不是无人值守全自动维护。
成品 OS 支持、NAS、Linux 部署和产品更新由后续原有 task 独立决定。
