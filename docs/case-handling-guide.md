# Case Handling Guide

Related docs:

- [Market Terms And Configuration](/Users/Philip/githome/lighting-match-engine-core/docs/market-terms-and-configuration.md)

## Goal

这份说明的目标是统一判断：

- 一个新 case 应该放在 `core` 里处理，还是放在外围
- 如果放在外围，外围应该怎么接入 `core`
- 如果放在 `core`，应该落在哪一层

当前建议是：

- `core` 保持窄
- 市场制度差异尽量外部化
- 只有“撮合内核必需”的能力才进入 `core`

## Core Responsibilities

`core` 负责这些能力：

- `MarketPhase` 状态机
- `AuctionKind` / `AuctionSession`
- `SessionRunner` 所驱动的基础 phase 流程
- `CallAuctionPool`
- 连续竞价 `OrderBook`
- 基础订单校验
  - tick
  - dense range
  - phase routing
- 基础成交结果
  - `Trade`
  - `MatchOutcome`

## Outer Responsibilities

外围负责这些能力：

- 交易时段调度
- 午间休市 / 夜盘 / 节假日
- 市场制度差异
- 策略型订单
- 风控 / 保证金 / 持仓
- 产品上市和退市编排
- 各类市场特有规则

外围的主要工作方式是：

1. 决定当前是否允许提交/撤单
2. 决定当前应该启动哪个 session
3. 把复杂订单转成 `core` 能理解的基础订单
4. 监听 `core` 输出结果，再决定下一步外部行为

## Decision Rule

判断一个 case 是否该进入 `core`，可以用这几个问题：

### 问题 1
如果没有这个能力，`core` 还能不能完成基础撮合？

- 如果不能，进入 `core`
- 如果能，优先放外围

### 问题 2
这个能力是不是可以通过“外围生成普通订单”实现？

- 如果可以，优先放外围
- 例如冰山单、止损单、TWAP

### 问题 3
这个规则是不是强依赖具体市场？

- 如果是，优先放外围
- 例如午间休市是否允许撤单

### 问题 4
这个规则是不是所有市场都必须共享？

- 如果是，可以考虑进 `core`
- 例如 phase 下不能撮合、dense tick 校验

## Typical Cases

### 1. 开盘集合竞价

处理位置：

- `core`

原因：

- 这是基础交易阶段能力
- 不属于某个市场的外围策略

当前落点：

- `AuctionKind::Opening`
- `MarketPhase::AuctionOrderEntry/Frozen/Matching`
- `CallAuctionPool`

### 2. 收盘集合竞价

处理位置：

- `core` + 外围配置

原因：

- 撮合机制本身属于 `core`
- 是否启用由 `MarketStructureConfig` 控制

### 3. 波动中断集合竞价

处理位置：

- `core` + 外围触发

原因：

- auction 机制属于 `core`
- 何时触发中断，通常由外围监控或市场控制器决定

### 4. 午间休市

处理位置：

- 外围

原因：

- 它主要是交易时间管理，不是撮合逻辑
- 不同市场规则差异大

外围做法：

- 不调用 `submit_order`
- 或只允许撤单
- 或缓存订单，等下一 session 再提交

### 5. 夜盘

处理位置：

- 外围

原因：

- 它是 session 编排问题
- 不需要改变撮合逻辑

外围做法：

- 把白盘和夜盘拆成两个 session

### 6. 冰山订单

处理位置：

- 外围

原因：

- 可以拆成普通 `Limit` 子单
- `core` 不需要理解“冰山”概念

外围做法：

- 外围持有母单
- 暴露一部分可见量到 `core`
- 成交后再补新子单

### 7. 止损单 / 条件单

处理位置：

- 外围

原因：

- 本质是触发逻辑，不是撮合内核

外围做法：

- 监听价格/成交
- 触发后生成普通 `Limit` 或 `Market` 单提交给 `core`

### 8. 做市商策略单

处理位置：

- 外围

原因：

- 是策略层，不是撮合层

### 9. 保证金 / 强平 / 风控

处理位置：

- 外围

原因：

- 这是账户和风险域
- 不应该污染撮合内核

外围做法：

- 提交前做风险检查
- 风险事件发生时，外围生成普通平仓单送进 `core`

### 10. 不同市场的时段安排

处理位置：

- 外围

原因：

- 这是 schedule 问题，不是 matching 问题

外围做法：

- 用 schedule/session planner 决定什么时候启动：
  - opening session
  - continuous session
  - closing session

### 11. 产品上市时的一次性集合竞价

处理位置：

- `core` + 外围 session 编排

原因：

- auction 能力复用 `core`
- “这是产品首次上市”是外围业务事件

外围做法：

- 启动一个 listing session
- 先进入 `Opening` auction
- 再进入 `ContinuousTrading`

### 12. 退市 / 最后交易日关闭

处理位置：

- 外围

原因：

- 这是产品生命周期控制
- `core` 只需要接收 `Closed`

## Recommended Handling Patterns

### Pattern A: Block Outside Core

适合：

- 午间休市
- 节假日闭市
- 非交易时段禁单

做法：

- 外围直接不调用 `submit_order`

### Pattern B: Transform Before Submit

适合：

- 冰山单
- 条件单
- 策略单

做法：

- 外围把复杂单转成基础订单
- 再调用 `submit_order`

### Pattern C: Trigger Session Transition

适合：

- 开盘
- 收盘
- 波动中断 auction
- 产品上市

做法：

- 外围调度器决定何时调用 session runner

### Pattern D: Consume Core Results And React

适合：

- 冰山补单
- 风险检查后续动作
- 行情派生行为

做法：

- 外围监听 `Trade` / `MatchOutcome`
- 再决定下一步提交什么单

## Market Examples

### A-share style

- 开盘集合竞价：`core`
- 午间休市：外围
- 连续竞价：`core`
- 收盘关闭：外围驱动到 `Closed`

### Nasdaq-style

- 开盘集合竞价：`core`
- 连续竞价：`core`
- 收盘集合竞价：`core`
- 波动中断触发：外围

### Crypto listing

- 上市事件：外围
- 上市集合竞价：`core`
- 长期连续交易：`core`
- 7x24 调度：外围

### Futures / commodities

- 日盘 / 夜盘划分：外围
- 开收盘 auction：按市场决定，`core` 可承接
- 午间休市：外围
- 风险和保证金：外围

## Practical Rule Of Thumb

如果一个新需求满足下面任一条，优先不要放进 `core`：

- 可以靠外围拆单实现
- 可以靠外围调度实现
- 明显是某个市场专属规则
- 明显属于账户/风控/策略层

只有在下面情况下，才应考虑进入 `core`：

- 不进入 `core` 就无法完成基础撮合
- 所有市场都必须共享
- 它直接改变 `phase routing` 或撮合正确性

## Current Code Mapping

- 术语和主类型：
  [data.rs](/Users/Philip/githome/lighting-match-engine-core/src/types/data.rs)
- 状态机和订单路由：
  [engine_state.rs](/Users/Philip/githome/lighting-match-engine-core/src/types/engine_state.rs)
- 集合竞价撮合：
  [call_auction_pool.rs](/Users/Philip/githome/lighting-match-engine-core/src/orderbook/call_auction_pool.rs)
- session 级流程驱动：
  [session_runner.rs](/Users/Philip/githome/lighting-match-engine-core/src/system/session_runner.rs)
