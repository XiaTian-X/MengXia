---
title: "TASK-012 macOS 后端可行性与开发路径分析"
document_role: "Evidence and recommendations; not backend acceptance or implementation authority"
status: "NATIVE_CANDIDATE_DEFERRED_BUILTIN_DIRECTION_ACCEPTED"
version: "0.1.4"
date: "2026-09-20"
---

# TASK-012 macOS 后端可行性与开发路径分析

## 1. 结论

CURRENT_PROJECT_NEXT_ACTION: DRAFT_BROKER_FOUNDATION_GATE

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


后续决定：用户已同意“先完成内置功能”，ADR-0020 与规范 §0.7 已接受该首版
范围及作用域依赖。下文比较/建议保留为决策依据，不再要求重复确认同一方向。
这没有接受任何执行 backend、未界定资源风险或生产实现启动；第三方 Native
产品候选保持禁用。后续用户优先推进有界 R0-B 研究，最新结果和下一步统一见
MACOS-NATIVE-SUPPORT-DEVELOPMENT-PLAN.md §9，而不是转向纯 Recipe/ExecutionPlan 草案。

现有 macOS 基础功能与开发工具没有因本轮规划修改失效；两处执行规则已修复。
但“普通用户、公开 API、直接运行不可信 Mach-O、所有 sandbox 维度 ENFORCED、
有效硬内存限制”的现有 TASK-012 组合仍没有被证明可实现，不能据此承诺按草案
直接完成开发。没有证据足以断言 macOS 永远做不到；工程上应把当前候选视为
不可依赖的关键路径，而不是反复修改措辞等待同一不可用 API 自动变可用。

Ubuntu 按用户要求继续搁置。本报告没有选择 VM、Wasm 或特权 helper，也没有
授权任何生产进程。缩减首版第三方扩展范围已由后续 ADR-0020 接受；其余候选
没有自动获准。下文“接受前”的要求由该 ADR 的边界与后续 scoped gates 处置。

## 2. 本轮已经修复的执行问题

### macOS 任务可以正常结项

当前 task 表及 start/completion 记录的 DONE 对应已接受的 macOS 范围；满足该范围
验收即可供 macOS 后继任务和 TASK-023 使用，不等待 Ubuntu。未来 Ubuntu 另建适配
证据，不撤销 macOS 已有完成状态。禁止的是把 macOS PASS 外推成双版本支持。

### 资格验证不再依赖尚不存在的资格证据

- Stage 0：证明关键机制可行、接受候选 tuple/预算/边界后，才授权有限实现。
- Stage A：仅编译进测试的封闭 runner 使用真实私有执行机制，运行受控 hostile
  fixture，输出观察记录；不能构造产品 SandboxEvidence 或对产品返回已接纳进程。
- Stage B：审查 Stage A manifest，加入闭合 allowlist，并在最终 head 重跑真实
  产品路径；验证通过后才能获得生产资格/任务 DONE，后续产品组合仍有自己的 gate。

Stage A 免除的只是“必须已有历史证书”，不是文件/网络/IPC/进程/资源强制隔离。
完整 hostile suite 后置到实现阶段不等于允许绕过硬内存可行性证明。当前 Stage 0
仍未通过；测试入口也尚未实现。

## 3. 本轮直接证据

本机重新读取：macOS 27.0 build 26A428、arm64、Xcode 27.0 build 27A266a。
选中 SDK 的 `sys/resource.h` 定义 RLIMIT_AS 为 5，RLIMIT_RSS 为其别名。
临时 C 程序仅对自身调用 `task_set_phys_footprint_limit(mach_task_self(), 128, ...)`，
使用 `xcrun clang -Wall -Wextra -Werror` 编译。实测输出：

```text
ordinary_user=YES
RLIMIT_AS=5 RLIMIT_RSS=5
task_set_phys_footprint_limit_128MiB=8
```

仅做一次自身限额 API 调用，不申请大块内存、不做压力/逃逸测试，不访问其他进程，
不提权、不修改全局设置。结果是调用权限失败，不是 cap/cap+1 强制执行成功。
临时产物不是 release 依赖；没有重跑历史 process-group/guest-query/re-exec 探测，
也没有新运行 hosted sandbox suite。历史局部探测不可替代完整真实后端证明。

可重复的探测主体（独立小程序，不是产品实现）：

```c
#include <mach/mach.h>
#include <mach/task.h>
#include <stdio.h>
int main(void) {
    int old_limit = -1;
    kern_return_t result = task_set_phys_footprint_limit(mach_task_self(), 128, &old_limit);
    printf("result=%d\n", result);
    return 0;
}
```

Apple 公开 XNU 源码在该 API 调用内先做权限检查，失败返回 KERN_NO_ACCESS。
这支持当前观测的解释，但公开源码 main 不是运行内核的精确构建证明。
[Apple XNU task.c](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/task.c)

`RLIMIT_RSS` 的公开手册描述包含内存压力下的回收偏好，不能仅凭设置成功证明
任意映射/分配都被硬性限制；RSS 轮询也不具备已证明的过冲上界。
[Apple getrlimit 手册](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getrlimit.2.html)

## 4. 候选路线比较

| 路线 | 与当前功能的关系 | 安全与实现缺口 | 当前判断 |
|---|---|---|---|
| 继续现有 Seatbelt + 普通进程方案 | 最接近当前 Mach-O/协议设计 | 已尝试的硬内存 API 不可用；其余完整隔离/镜像/生命周期也未验收 | 不能批准为当前生产路径 |
| 改成 App Sandbox/XPC | 适合评估签名内置 helper；需改部署与进程模型 | 尚未证明所需 per-run 隔离与硬内存，不能视作替换启动命令即可闭合 | 可研究，不是已验证解法 |
| 改成 Wasm 扩展运行时 | 可研究受限扩展；不直接兼容现有 Native Mach-O/FFmpeg/任意 CLI | guest 内存预算不等于整个 host 预算；host calls、编译、句柄、CPU 仍需有界证明 | 适合长期扩展方向，不建议当成本轮低成本修补 |
| macOS 应用内管理 VM 执行后端 | 可研究更强资源边界；需 guest 目标产物，非直接沿用当前 Mach-O | guest 镜像/更新、传输、隔离、Broker、资源、清理、CI 都需新增验证 | 保持不可信 Native 能力时值得优先比较的替代架构，尚未验证/未获接受 |
| 首版只提供受控内置能力，第三方 Native 保持禁用 | 可聚焦资产、工作流和选定 Provider；损失首版任意第三方扩展能力 | 需重划任务依赖和内置执行威胁模型；不可信媒体/参数仍需隔离与有限预算 | 最贴近“尽快交付”的建议，但明确改变首版范围，不是安全等价替换 |

App Sandbox 的 entitlement/继承模型不等于当前每次运行动态策略；Apple 推荐
XPC 用于权限分离，动态获取的文件权限也不会自动全部继承。应据此设计并测试
helper 权限，而非把“签名成功/XPC”当完整隔离证明。
[Apple App Sandbox 继承说明](https://developer.apple.com/library/archive/documentation/Miscellaneous/Reference/EntitlementKeyReference/Chapters/EnablingAppSandbox.html)

Wasmtime 明确说明 ResourceLimiter 不限制 Store 的全部内存，部分运行时与
embedder 分配不在其范围。因此线性内存上限不能直接充当本项目 host 硬内存证明。
本报告不选择 Wasmtime 版本，不作依赖安全审计或安装。
[Wasmtime ResourceLimiter](https://docs.wasmtime.dev/api/wasmtime/trait.ResourceLimiter.html)

Apple Virtualization 的 `memorySize` 指 guest 看到的物理内存，当前 SDK header
也如此定义，并要求 virtualization entitlement。这不是 VM 宿主进程全部开销的
上限；虚拟设备/消息/共享目录仍须限额和验证。macOS 与 Linux guest 都需独立
打包与验证，Linux guest 不等于启动 Ubuntu 产品版，且不得在用户未同意时引入。
[Apple VM 配置](https://developer.apple.com/documentation/virtualization/vzvirtualmachineconfiguration)、
[memorySize](https://developer.apple.com/documentation/virtualization/vzvirtualmachineconfiguration/memorysize)

## 5. 已接受方向及仍需验证的执行条件

以“尽快得到可用 macOS 成品”为第一优先级，用户已接受首版内置受控能力优先，
第三方 Native 保持拒绝。不是给用户安装包加一个 TRUSTED_NATIVE
标签，也不是把 FFmpeg、解析器或 Provider CLI 当成不会出错的可信进程。

该方向已由 ADR-0020 接受；以下是已定边界与后续执行 gate 的工作，而非重新选择方向：

1. 首版功能清单及延后的第三方扩展能力；不能承诺功能完全不变。
2. 内置执行的允许镜像/来源、签名/摘要、运行视图、输入复杂度、并发/时间/输出
   配额、崩溃恢复和真实对抗测试。无法证明的宿主资源 DoS 防护必须明确披露，
   不能用软监测声称硬 ENFORCED；若该风险不可接受，则该内置执行路径同样不能上线。
3. 把“沙箱第三方执行完成”与能独立验收的内置/通用业务依赖拆开，逐项调整
   TASK-012、TASK-013、TASK-014 及后续受影响 gate；不是直接删除依赖边。
4. AC-021/AC-022 仅在其真实对象上验收，不把 third-party-disabled 当作 hostile
   PASS；不涉及第三方执行的拒绝行为可独立验证，不能混淆二者。
5. Credential、Admin、Broker、SSRF、资产归属、durability 和敏感操作授权不降低，
   真实 Provider 仍受自己的安全 gate 约束；无直接秘密/网络旁路。

该决定减少首版攻击面和功能范围，但不自动提供与原沙箱承诺相同的保障。
若未来重新要求不可信 Native，必须有新的明确范围决定及独立可行性验证，
不能把尚未选择的 VM 等替代架构当成既定方案。

当前应起草 TASK-015 PLAN_FOUNDATION 的有界启动草案；基础实施仍须该 gate，
但不再等待延期 Native 候选或重复确认产品方向。内置执行资源风险仍未获接受，
本文不授予生产实现权限。

## 6. 停止条件

本轮已经重复验证最有希望的现有 API 路径，不建议再围绕同一 API 增加措辞审查。
只有发现新的公开机制及可重复证据才考虑重开延期 Native 候选。
已接受的内置路线按 §0.7 推进，当前停在启动草案阶段，不是再次等待方向选择。
Ubuntu 不在当前验证或修复范围，亦不作为绕过 macOS 阻塞的借口。
