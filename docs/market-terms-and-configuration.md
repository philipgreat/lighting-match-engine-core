# Market Terms And Configuration

Related docs:

- [Case Handling Guide](/Users/Philip/githome/lighting-match-engine-core/docs/case-handling-guide.md)

## Glossary

### `MarketPhase`
交易引擎当前所处的市场阶段。它决定订单应该被拒绝、进入集合竞价池，还是进入连续竞价订单簿。

当前代码中的阶段有：

- `PreOpen`
  系统已启动，但还没有进入可提交订单的交易阶段。
- `AuctionOrderEntry(AuctionKind)`
  集合竞价收单阶段。订单进入 `active_auction.pool`。
- `AuctionFrozen(AuctionKind)`
  集合竞价冻结阶段。通常不再接收新订单，准备计算成交价。
- `AuctionMatching(AuctionKind)`
  集合竞价撮合阶段。只允许执行集合竞价撮合。
- `ContinuousTrading`
  连续竞价阶段。订单进入连续竞价 `order_book`。
- `TradingHalt`
  停牌或中断阶段。当前实现里预留了状态，但还没有扩展细则。
- `Closed`
  该次交易 session 已结束。

### `AuctionKind`
集合竞价的类型。它描述“这次集合竞价是为什么存在”，而不是描述撮合算法本身。

当前代码中的类型有：

- `Opening`
  开盘集合竞价
- `Closing`
  收盘集合竞价
- `VolatilityInterruption`
  波动性中断集合竞价

### `CallAuctionPool`
当前集合竞价 session 使用的订单池。只有在 `AuctionOrderEntry(...)` / `AuctionFrozen(...)` / `AuctionMatching(...)` 这几类阶段下才应该有业务意义。

### `AuctionSession`
一次具体的集合竞价 session。它持有：

- `kind`
- `pool`
- `started_at`
- `frozen_at`
- `matched_at`
- `last_outcome`

它不是永久存在的全局池，而是某一轮集合竞价的上下文对象。

### `MarketStructureConfig`
市场结构配置。它描述“这个市场支持哪些阶段”，而不是描述“当前处于哪个阶段”。

当前字段：

- `has_opening_auction`
- `has_closing_auction`
- `allows_volatility_auction`

### `SessionRunner`
驱动一个 session 生命周期的模块。它负责按照预定义流程推进 `MarketPhase`，并在合适的阶段提交订单、执行集合竞价。

当前实现位于：

- [session_runner.rs](/Users/Philip/githome/lighting-match-engine-core/src/system/session_runner.rs)

### `SessionSummary`
一次 session runner 执行后的汇总结果。当前包含：

- `opening_auction`
- `closing_auction`

这是一种演示型 summary，不代表未来最终形态必须固定为这两个字段。

## Configuration Principles

### 原则 1
`MarketStructureConfig` 只定义能力，不定义排程。

例如：

- “支持收盘集合竞价”是能力
- “今天 15:57 开始收盘集合竞价”是排程

### 原则 2
`MarketPhase` 是运行时状态，`SessionRunner` 是状态推进器。

也就是说：

- 配置层决定能不能进入某种 phase
- runner 决定什么时候进入该 phase
- `EngineState` 负责在该 phase 下如何路由订单

### 原则 3
“集合竞价”应该被视为一个可重复使用的 session 模板，而不是某个市场专用的一次性模块。

## Recommended Configurations By Market

### 1. A-share style
典型流程：

- `PreOpen`
- `AuctionOrderEntry(Opening)`
- `AuctionFrozen(Opening)`
- `AuctionMatching(Opening)`
- `ContinuousTrading`
- `Closed`

推荐配置：

```rust
MarketStructureConfig {
    has_opening_auction: true,
    has_closing_auction: false,
    allows_volatility_auction: false,
}
```

说明：

- 有开盘集合竞价
- 没有收盘集合竞价
- 不启用波动性中断集合竞价

### 2. Nasdaq-style equity market
典型流程：

- `PreOpen`
- `AuctionOrderEntry(Opening)`
- `AuctionFrozen(Opening)`
- `AuctionMatching(Opening)`
- `ContinuousTrading`
- `AuctionOrderEntry(Closing)`
- `AuctionFrozen(Closing)`
- `AuctionMatching(Closing)`
- `Closed`

推荐配置：

```rust
MarketStructureConfig {
    has_opening_auction: true,
    has_closing_auction: true,
    allows_volatility_auction: true,
}
```

说明：

- 有开盘集合竞价
- 有收盘集合竞价
- 可支持波动性中断后重新进入集合竞价

### 3. Crypto listing session
典型流程：

- `PreOpen`
- `AuctionOrderEntry(Opening)`
- `AuctionFrozen(Opening)`
- `AuctionMatching(Opening)`
- `ContinuousTrading`

推荐配置：

```rust
MarketStructureConfig {
    has_opening_auction: true,
    has_closing_auction: false,
    allows_volatility_auction: false,
}
```

说明：

- “Opening” 在这里不表示交易日开盘，而表示“该产品的首次开市集合竞价”
- 进入 `ContinuousTrading` 后可以长时间保持，不必按日结束
- 这类场景更适合使用 `SessionRunner` / `LifecycleRunner`，而不是强调“day runner”

### 4. Crypto always-on market without listing auction
典型流程：

- `PreOpen`
- `ContinuousTrading`

推荐配置：

```rust
MarketStructureConfig {
    has_opening_auction: false,
    has_closing_auction: false,
    allows_volatility_auction: false,
}
```

说明：

- 不经过任何集合竞价
- 系统启动后可以直接从 `PreOpen` 进入 `ContinuousTrading`

### 5. Market with volatility interruption auctions
典型流程：

- `ContinuousTrading`
- `TradingHalt`
- `AuctionOrderEntry(VolatilityInterruption)`
- `AuctionFrozen(VolatilityInterruption)`
- `AuctionMatching(VolatilityInterruption)`
- `ContinuousTrading`

推荐配置：

```rust
MarketStructureConfig {
    has_opening_auction: true,
    has_closing_auction: true,
    allows_volatility_auction: true,
}
```

说明：

- `VolatilityInterruption` 不是新的撮合算法
- 它只是把现有集合竞价能力复用到中断恢复场景

## Practical Guidance

### 如果市场差异只在“有没有某种 auction”
只改 `MarketStructureConfig`。

### 如果市场差异在“什么时候进入某个 phase”
改 `SessionRunner`。

### 如果市场差异在“某个 phase 下订单怎么处理”
改 `EngineState` 的 phase routing 规则。

### 如果市场差异在“集合竞价成交价怎么选”
改 `CallAuctionPool` 的定价规则，不要把这类逻辑塞进 runner。

## Current Code Mapping

- 市场状态和配置：
  [data.rs](/Users/Philip/githome/lighting-match-engine-core/src/types/data.rs)
- 状态迁移和订单路由：
  [engine_state.rs](/Users/Philip/githome/lighting-match-engine-core/src/types/engine_state.rs)
- 集合竞价定价和撮合：
  [call_auction_pool.rs](/Users/Philip/githome/lighting-match-engine-core/src/orderbook/call_auction_pool.rs)
- session 级流程驱动：
  [session_runner.rs](/Users/Philip/githome/lighting-match-engine-core/src/system/session_runner.rs)
