# Customer Validation Log Template

## 1. 目标

这份模板用于记录外部客户触达、设计合作客户访谈、PoC 推进与复盘结果，作为 GTM 与产品收敛的统一证据载体。

## 2. 使用场景

适用于以下活动后的记录：

- 首次外联
- 需求访谈
- 标准 demo 演示
- 安全评估沟通
- PoC 周会
- PoC 结束复盘

## 3. 记录字段

| 字段 | 说明 |
| --- | --- |
| Date | 日期 |
| Account | 客户、团队或 BU |
| Stage | `outreach`, `intro`, `discovery`, `demo`, `security_review`, `poc`, `post_poc` |
| Participants | 参与人 |
| Use Case | 当前验证场景 |
| Top Questions | 客户最关心的 3 到 5 个问题 |
| Current Answers | 当前统一回答 |
| Risks | 当前阻塞或风险 |
| Objections Ref | 对应 objections 记录 |
| Decision | `advance`, `hold`, `stop`, `follow_up` |
| Next Step | 下一步动作 |
| Owner | 负责人 |

## 4. 最低记录要求

每次记录至少应包含：

- 一个明确场景
- 三个以上客户问题
- 当前回答
- 下一步动作
- owner

## 5. 周度复盘建议

每周至少汇总：

- 本周新增客户触达数
- 进入 demo / PoC 的数量
- Top 5 关注问题
- 当前最常见 objections
- 需要新增的文档、测试或功能

## 6. 示例

| Date | Account | Stage | Participants | Use Case | Top Questions | Current Answers | Risks | Objections Ref | Decision | Next Step | Owner |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-03-29 | Example Bank | demo | Platform Lead, Security Lead | PR review agent | 是否支持私有化部署；默认安全边界是什么；审计能记录什么 | 支持 Linux 私有化部署；默认限制网络；提供 request_id、tenant_id 与审计事件 | 安全团队要求更明确的宿主保护说明 | `docs/product/objections-log-template.md` | follow_up | 发送安全白皮书并安排 PoC 条件确认 | product |
