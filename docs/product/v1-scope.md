# Synapse v1 Scope

## 定位

Synapse v1 的产品定位固定为：

**企业内 AI 执行平面。**

它服务的不是通用开发者沙箱市场，而是企业内部 AI 代码助手、PR Review Agent、自动化工作流所需的受控执行层。

## 核心承诺

v1 只承诺以下能力：

- Python 受控执行
- 稳定的 HTTP 与 websocket 执行 API
- 基础多租户隔离与配额控制
- 审计记录与请求追踪
- Linux 私有化部署
- 最薄官方 Python SDK
- 面向 PoC 的标准 demo 与接入文档

## 核心使用场景

主场景：

- 企业内部 AI 代码助手 / Agent 执行层

次级演示场景：

- PR Review Agent
- AI 驱动的 CI/CD 自动化

## 交付边界

v1 的交付必须包含：

- `docs/api-reference.md` 中定义的 API v1 契约
- `sdk/python/` 可直接用于最小接入
- `docs/quickstart/enterprise-poc-guide.md` 的部署与联调路径
- `docs/product/security-whitepaper.md` 的安全边界说明
- `examples/pr-review-agent/` 的标准 demo
- `scripts/release_gate_v1.sh` 的发布门禁

## 验收标准

- 客户可在 1 天内完成基础部署
- 客户可在 1 周内接入一个真实 agent 场景
- 销售、方案、研发可使用同一套文档和 demo 演示
- 首版演示不依赖临时手工修补
