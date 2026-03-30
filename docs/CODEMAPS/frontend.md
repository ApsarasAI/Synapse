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
- 控制台为 `synapse-api` 内嵌单文件页面：
  - `crates/synapse-api/src/admin_console.rs`
  - `crates/synapse-api/src/admin_console.html`

## 状态管理流
```
CLI args
  -> rust enum Commands
  -> serve / doctor / runtime

HTTP state
  -> AppState
  -> SandboxPool
  -> in-memory metrics

Embedded admin console
  -> sessionStorage bearer token
  -> fetch /admin/overview
  -> fetch /admin/requests
  -> fetch /admin/requests/:request_id
  -> fetch /admin/requests/:request_id/audit
  -> fetch /admin/runtime
  -> fetch /admin/capacity
```

## 结论
- 当前代码库已包含一个内嵌只读运维控制台
- 页面未引入独立前端工程，适合作为最小运维入口
- 如果后续需要更复杂的交互或构建链，再拆分到新 crate 或独立目录
