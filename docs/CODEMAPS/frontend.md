<!-- Generated: 2026-03-25 | Files scanned: 24 | Token estimate: ~180 -->
# 前端架构

## 页面树
- `GET /admin/console`
  - `Dashboard`
  - `Requests`
  - `Execution Detail`
  - `Runtime`

## 组件层级
- 无 React/Vue/Svelte 组件
- 控制台页面资源已拆到独立 crate：
  - `crates/synapse-console/src/lib.rs`
  - `crates/synapse-console/src/admin_console.html`
- `synapse-api` 仅负责暴露 `/admin/console` 路由与数据接口

## 状态管理流
```
CLI args
  -> rust enum Commands
  -> serve / doctor / runtime

HTTP state
  -> AppState
  -> SandboxPool
  -> in-memory metrics

synapse-console
  -> sessionStorage bearer token
  -> fetch /admin/overview
  -> fetch /admin/requests
  -> fetch /admin/requests/:request_id
  -> fetch /admin/requests/:request_id/audit
  -> fetch /admin/runtime
  -> fetch /admin/capacity
```

## 结论
- 当前代码库已包含一个只读运维控制台前端 crate
- 页面仍是单文件 HTML/JS，但资源已从 API crate 中解耦
- 如果后续需要更复杂的交互或构建链，可继续在 `synapse-console` 内演进或再拆分独立前端工程
