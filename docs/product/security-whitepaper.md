# Synapse v1 Security Whitepaper

## 1. 文档目标

本文档用于帮助客户的安全、平台和审计团队理解 Synapse v1 的默认安全边界、已覆盖能力与已知限制。

## 2. 默认安全边界

Synapse v1 面向 Linux 主机的受控代码执行，默认边界包括：

- 受控 runtime 执行
- 超时控制
- CPU 与内存限制
- 多租户配额限制
- 审计记录
- 默认禁用网络访问策略扩展

Synapse 的目标不是让任意代码“完全可信”，而是在企业可接受的边界内限制和追踪执行行为。

## 3. 默认拒绝项

当前版本默认不承诺以下能力：

- 对外网络白名单放通
- 浏览器或桌面自动化
- 多语言 runtime 治理
- 面向公网多租户 SaaS 的托管安全责任

对于 `network_policy.mode = "allow_list"`，当前实现会直接拒绝请求。

## 4. 宿主保护方式

部署方应提供以下宿主条件：

- Linux 环境
- `bwrap`
- `strace`
- cgroup v2
- 受控 Python runtime

Synapse 依赖宿主提供基础内核与隔离能力，并在应用层增加资源、审计和租户控制。

## 5. 审计覆盖范围

当前版本的审计面向执行链路，至少覆盖：

- request 接收
- tenant / quota 准入
- runtime 解析
- 执行结果
- 失败语义
- sandbox reset

PoC 与生产接入时，客户应验证审计记录与其内部日志链路的对接要求。

## 6. 失败语义

v1 对外暴露明确错误码，重点包括：

- `invalid_input`
- `runtime_unavailable`
- `queue_timeout`
- `capacity_rejected`
- `wall_timeout`
- `cpu_time_limit_exceeded`
- `memory_limit_exceeded`
- `sandbox_policy_blocked`
- `quota_exceeded`
- `rate_limited`
- `auth_required`
- `auth_invalid`
- `tenant_forbidden`
- `audit_failed`
- `io_error`
- `execution_failed`

这些错误码是客户接入、告警和审计判定的基础。

## 7. 已知限制

- 当前官方 SDK 仅覆盖 Python
- 运行时仍以 Python 为主
- 当前没有托管控制面和自助门户
- 当前不覆盖浏览器、桌面与复杂长生命周期 sandbox 能力

## 8. 客户侧控制建议

- 在专用主机或专用节点池部署 Synapse
- 把 API token 与 tenant 权限纳入现有密钥管理
- 将审计日志纳入内部 SIEM 或审计系统
- 通过发布门禁脚本固化升级前验证
