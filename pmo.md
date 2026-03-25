# Synapse — 研发排期

> 渐进式开发：每个 Sprint 产出可运行、可验证的增量，持续集成、持续交付。

---

## 总览

```
Phase 1: MVP 核心验证（4 周 / 8 个 Sprint）
  Sprint 1-2: 项目骨架 + 最小沙箱
  Sprint 3-4: 文件系统隔离 + Seccomp
  Sprint 5-6: 沙箱预热池 + HTTP API
  Sprint 7-8: Python 执行 + 集成测试 + v0.1.0 发布

Phase 2: 安全加固与开发者体验（3 周 / 6 个 Sprint）
  Sprint 9-10: Cgroups + 网络隔离
  Sprint 11-12: 自适应 Seccomp + User Namespace 加固
  Sprint 13-14: WebSocket 流式输出 + Python SDK

Phase 3: 商业化与高性能（4 周 / 8 个 Sprint）
  Sprint 15-16: 多语言运行时 + Synapsefile
  Sprint 17-18: 审计日志 + 多租户
  Sprint 19-20: SaaS 控制台 + 计费
  Sprint 21-22: io_uring 优化 + Beta 发布

Phase 4: 长期演进（持续）
```

每个 Sprint = 2.5 个工作日。每个 Sprint 结束时有明确的可验证交付物。

---

## Phase 1：MVP 核心验证（第 1-4 周）

```
目标：跑通 "POST /execute → Python 代码执行 → JSON 返回" 完整链路
交付物：GitHub v0.1.0，单二进制文件，< 5ms 冷启动（预热池）
```

### Sprint 1（第 1 周上半）— 项目骨架

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| 初始化 Cargo workspace（synapse-core / synapse-api / synapse-cli） | 项目编译通过 | `cargo build` |
| synapse-cli 基础命令框架（serve / runtime / doctor） | CLI help 输出正确 | `synapse --help` |
| CI 流水线搭建（GitHub Actions：clippy + fmt + test） | PR 自动检查 | 提交 PR 触发 |
| README.md + LICENSE-APACHE | 项目可公开 | 人工检查 |

交付物：`cargo build` 通过，CI 绿灯，空壳项目可运行。

### Sprint 2（第 1 周下半）— 最小沙箱

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| namespace 模块：clone() + PID/Mount/IPC/UTS/Network Namespace | 子进程在独立 Namespace 中运行 | 单元测试：子进程 `getpid()` 返回 1 |
| User Namespace + UID/GID 映射（root → nobody） | 沙箱内 root 在宿主机为 nobody | 集成测试：`/proc/self/uid_map` 验证 |
| 最小 Sandbox 结构体 + SandboxState 状态机 | 可创建/销毁沙箱 | 单元测试 |

交付物：`Sandbox::new()` 可创建隔离子进程，`Sandbox::destroy()` 可清理。

### Sprint 3（第 2 周上半）— 文件系统隔离

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| OverlayFS 挂载/卸载封装 | 只读层 + 可写层正确合并 | 集成测试：写入可写层，只读层不变 |
| pivot_root + 虚拟文件系统挂载（/proc, /dev, /tmp） | 沙箱根目录切换成功 | 集成测试：沙箱内 `ls /` 只看到预期目录 |
| 文件系统隔离验证 | 沙箱无法访问宿主机文件 | 安全测试：`open("/etc/passwd")` 返回 ENOENT |

交付物：沙箱拥有独立文件系统，宿主机文件不可见。

### Sprint 4（第 2 周下半）— Seccomp 过滤

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| libseccomp 集成 + SeccompProfile 结构体 | Profile 可加载到进程 | 单元测试 |
| Minimal / Standard 两个预定义 Profile | 白名单系统调用列表确定 | 代码 review |
| Seccomp 拦截验证 | fork/execve/socket 被阻止 | 安全测试：`escape_fork_bomb`、`escape_network` |

交付物：沙箱内危险系统调用被拦截，SIGSYS 终止进程。

### Sprint 5（第 3 周上半）— 沙箱预热池

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| SandboxPool 数据结构（VecDeque + Mutex） | 池创建/获取/回收 | 单元测试 |
| acquire / release / replenish 核心算法 | 池自动补充 + 降级 | 单元测试：池耗尽后降级创建 |
| PoolMetrics 指标 | pool_size / active / poisoned 可观测 | 指标端点验证 |
| 后台 replenish tokio task | 低水位自动补充 | 集成测试：消耗后观察补充 |

交付物：预热池工作正常，acquire < 0.1ms。

### Sprint 6（第 3 周下半）— HTTP API

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| axum Server 启动 + 路由注册 | 服务可启动监听 | `curl /health` 返回 200 |
| `POST /execute` 接口（请求解析 + 参数校验） | 接口可调用 | curl 测试 |
| `GET /health` + `GET /metrics` | 运维接口可用 | curl 测试 |
| 错误响应格式统一（ApiError → JSON） | 错误格式一致 | 边界输入测试 |

交付物：HTTP API 可接收请求，返回 mock 响应。

### Sprint 7（第 4 周上半）— Python 执行

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| RuntimeManager + python-base 运行时构建 | Python 只读层可用 | `synapse runtime list` |
| Sandbox::execute() 实现（写入代码 + execvp + 收集输出） | 代码可执行 | `print("hello")` 返回正确 |
| 超时处理（SIGKILL）+ OOM 处理 | 异常场景正确返回 | `while True: pass` 超时测试 |
| 输出截断（max 1MB） | 大输出不撑爆内存 | 大输出测试 |

交付物：`POST /execute` 可执行 Python 代码并返回结果。

### Sprint 8（第 4 周下半）— 集成测试 + v0.1.0 发布

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| 集成测试套件（完整生命周期 / 隔离验证 / API 端点） | 测试全部通过 | `cargo test --test integration` |
| 安全测试套件（8 个逃逸测试用例） | 全部拦截成功 | `cargo test --test security` |
| 性能基准测试（criterion：sandbox_create / pool_acquire / execute_hello） | 基线数据建立 | `cargo bench` |
| synapse doctor 命令 | 系统检查通过 | 在目标机器运行 |
| 静态链接构建（musl x86_64 + arm64） | 二进制文件可分发 | 在干净 Linux 上运行 |
| GitHub Release v0.1.0 | 版本发布 | 下载 + 运行验证 |

交付物：v0.1.0 发布，完整功能可用，测试全绿。

---

## Phase 2：安全加固与开发者体验（第 5-7 周）

```
目标：白名单 Seccomp + 自适应 Profile + WebSocket + SDK
交付物：开发者预览版 API，Python SDK 发布
```

### Sprint 9（第 5 周上半）— Cgroups v2 资源限制

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| CgroupManager 实现（memory.max / cpu.max / pids.max） | 资源限制生效 | 集成测试 |
| memory.swap.max = 0（禁用 swap） | 内存限制精确 | OOM 测试 |
| cgroup.kill 清理 + 目录删除 | 资源组正确释放 | 沙箱销毁后检查 /sys/fs/cgroup |
| PID 限制验证 | fork bomb 被阻止 | 安全测试 |

### Sprint 10（第 5 周下半）— 网络隔离

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| 断网模式完善（Network Namespace 空栈） | 沙箱内无网络 | `socket()` 调用失败 |
| 安全网络模式设计 + 实现（veth pair + nftables 白名单） | 仅允许指定 IP 出站 | curl 白名单 IP 成功，其他失败 |
| 网络模式配置化（`network: "none" / "restricted" / "full"`） | API 支持网络参数 | API 测试 |

### Sprint 11（第 6 周上半）— 自适应 Seccomp

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| Seccomp 切换为白名单模式（默认 deny） | 非白名单调用被拦截 | 安全测试 |
| Profile 模板库（Minimal / Standard / Scientific / Network） | 4 个预定义 Profile | 各 Profile 下 Python 代码正确执行 |
| 代码静态分析器（解析 Python import 语句） | 提取依赖列表 | 单元测试：`import numpy` → `["numpy"]` |
| import → Profile 映射（numpy → Scientific） | 自动选择 Profile | 集成测试 |
| `--learn` 模式（strace 跟踪 + profile 生成） | 自动生成最小 profile | 对比手写 profile 的调用数量 |

### Sprint 12（第 6 周下半）— User Namespace 加固

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| Capabilities 全部 drop | 沙箱无任何 capability | `capsh --print` 验证 |
| no_new_privs 设置 | 无法通过 exec 获取新权限 | 安全测试 |
| /proc 和 /sys 只读挂载加固 | 无法写入 /proc | 安全测试 |
| 安全测试套件扩充（提权攻击 / ptrace / symlink） | 全部拦截 | `cargo test --test security` |

### Sprint 13（第 7 周上半）— WebSocket 流式输出

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| `WS /execute/stream` 端点实现 | WebSocket 连接可建立 | wscat 测试 |
| StreamEvent 协议（stdout / stderr / done / error） | 消息格式正确 | 协议验证 |
| execute_streaming（pipe 非阻塞读取 + channel 转发） | 逐行推送输出 | `for i in range(5): print(i)` 逐行收到 |
| 超时 / OOM / 断开连接的边界处理 | 异常场景正确关闭 | 边界测试 |

### Sprint 14（第 7 周下半）— Python SDK + 发布

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| synapse-sdk Python 包（HTTP 封装 + 重试 + 连接池） | `pip install synapse-sdk` | 安装 + 调用测试 |
| SDK 流式输出支持（WebSocket 封装） | `sandbox.execute_stream()` | 流式输出测试 |
| API 文档（OpenAPI spec 自动生成） | Swagger UI 可访问 | 浏览器打开 |
| 开发者预览版发布 | v0.2.0-preview | GitHub Release |

---

## Phase 3：商业化与高性能（第 8-11 周）

```
目标：多语言 + 审计 + SaaS 控制台 + 500+ QPS
交付物：Beta 版上线，Enterprise 版本发布
```

### Sprint 15（第 8 周上半）— 多语言运行时

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| Node.js 运行时构建（nodejs-base 只读层） | Node.js 代码可执行 | `console.log("hello")` |
| Shell 运行时构建（shell-alpine 只读层） | Shell 脚本可执行 | `echo hello` |
| RuntimeManager 多运行时切换 | language 参数路由正确 | API 测试 |

### Sprint 16（第 8 周下半）— Synapsefile + 运行时管理

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| Synapsefile YAML 解析器 | 配置文件解析正确 | 单元测试 |
| `synapse runtime build` 命令 | 自定义运行时可构建 | 构建 python-scientific |
| `synapse runtime list / remove / export / import` | 运行时生命周期管理 | CLI 测试 |

### Sprint 17（第 9 周上半）— 审计日志

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| synapse-core 审计 hook 点（trait AuditHook） | Core 层预留扩展点 | 编译通过 |
| synapse-enterprise audit 模块实现 | 文件/网络/执行事件记录 | 执行代码后检查审计日志 |
| 审计日志输出格式（JSON Lines） | 结构化日志可采集 | jq 解析验证 |

### Sprint 18（第 9 周下半）— 多租户

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| API Key 鉴权中间件 | 无 Key 返回 401 | curl 测试 |
| 租户配额管理（并发 / RPM / 每日总量） | 超配额返回 429 | 压力测试 |
| 租户级沙箱隔离（并发上限） | 租户 A 不影响租户 B | 并发测试 |

### Sprint 19（第 10 周上半）— SaaS 控制台

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| 用户注册 / 登录 | 账号系统可用 | 浏览器测试 |
| API Key 管理（创建 / 吊销 / 列表） | Key CRUD 可用 | 控制台操作 |
| 用量统计仪表盘 | 执行次数 / 延迟图表 | 控制台查看 |

### Sprint 20（第 10 周下半）— 计费系统

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| 执行计量（每次 API 调用计数） | 用量准确 | 对比 Metrics |
| 套餐管理（Free / Pro / Team） | 套餐限制生效 | 超出 Free 额度被拒 |
| License Key 验证（私有化部署） | License 过期后拒绝启动 | 过期 Key 测试 |

### Sprint 21（第 11 周上半）— io_uring 优化

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| io_uring 集成（进程 IO 收集 + 超时控制） | 替换 tokio spawn 模型 | 功能回归测试 |
| 性能对比基准测试 | QPS 提升数据 | wrk 压力测试 |
| 上下文切换对比 | 减少 80%+ | perf stat 对比 |

### Sprint 22（第 11 周下半）— Beta 发布

| 任务 | 产出 | 验证方式 |
| :--- | :--- | :--- |
| 全量 E2E 测试 | 所有场景通过 | 测试报告 |
| 压力测试（4C8G，目标 500 QPS） | 达标 | wrk/k6 报告 |
| Node.js SDK 发布 | `npm install @synapse/sdk` | 安装 + 调用 |
| Enterprise 版本构建 + License 集成 | Enterprise 二进制可分发 | 功能验证 |
| Beta 版正式上线 | v1.0.0-beta | 公告发布 |

---

## Phase 4：长期演进（第 12 周起）

```
目标：WASM 双引擎 + 生态拓展 + 安全认证
无固定 Sprint，按季度规划
```

| 季度 | 重点 | 交付物 |
| :--- | :--- | :--- |
| Q1 | WASM 引擎集成（Wasmtime）+ 智能路由 | 双引擎 Alpha 版 |
| Q1 | gRPC 接口 | gRPC API 可用 |
| Q2 | OpenAI Function Calling 适配 | Agent 集成示例 |
| Q2 | 更多语言运行时（Go / Java / Ruby） | 运行时仓库扩充 |
| Q3 | 外部渗透测试 + 安全认证准备 | 安全审计报告 |
| Q3 | Edge Computing 探索（浏览器端 WASM） | 技术 PoC |
| Q4 | 按 CPU 时间计费 | 计费模式升级 |
| Q4 | 运行时仓库公开（`synapse runtime pull`） | 仓库上线 |

---

## 里程碑总览

```
Week 1  ──── Sprint 1-2 ──── 最小沙箱可运行
Week 2  ──── Sprint 3-4 ──── 文件系统 + Seccomp 隔离完成
Week 3  ──── Sprint 5-6 ──── 预热池 + HTTP API 可用
Week 4  ──── Sprint 7-8 ──── ★ v0.1.0 发布（MVP）
Week 5  ──── Sprint 9-10 ─── Cgroups + 网络隔离
Week 6  ──── Sprint 11-12 ── 自适应 Seccomp + 安全加固
Week 7  ──── Sprint 13-14 ── ★ v0.2.0-preview（WebSocket + SDK）
Week 8  ──── Sprint 15-16 ── 多语言 + Synapsefile
Week 9  ──── Sprint 17-18 ── 审计日志 + 多租户
Week 10 ──── Sprint 19-20 ── SaaS 控制台 + 计费
Week 11 ──── Sprint 21-22 ── ★ v1.0.0-beta（商业化上线）
Week 12+ ─── Phase 4 ─────── WASM / gRPC / 生态拓展
```

---

## 风险与缓冲

| 风险 | 影响 | 缓冲策略 |
| :--- | :--- | :--- |
| Seccomp 白名单调试耗时 | Sprint 4 延期 | 先用黑名单模式上线，白名单延到 Phase 2 |
| OverlayFS 在不同内核版本行为差异 | Sprint 3 延期 | 准备 bind mount 降级方案 |
| Python 运行时只读层构建复杂 | Sprint 7 延期 | 先用 chroot + 手动复制，OverlayFS 后续优化 |
| io_uring 内核版本要求高（>= 5.1） | Sprint 21 受限 | 保留 tokio spawn 作为 fallback |
| 单人开发精力有限 | 整体延期 | Phase 1 严格 4 周，Phase 2/3 可弹性延长 1 周 |