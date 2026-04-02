# Match Engine As Redis Module Design

## Goal

Turn the current project into:

- a reusable matching engine core library
- a Redis Module host
- a FIX business-message adapter used through Redis commands

Out of scope:

- FIX session management
- Logon / Logout / Heartbeat / ResendRequest
- UDP multicast transport
- persistence and recovery

## External Contract

The transport protocol exposed to clients is Redis RESP.

FIX is only used as the business payload carried inside Redis commands.

The module will accept FIX business messages and return FIX business messages or compact Redis replies.

## Command Surface

Primary command:

- `FIX.SEND <product_id> <fix_message>`

Read-only commands:

- `FIX.BOOK <product_id>`
- `FIX.STATS <product_id>`
- `FIX.RESET <product_id>`

## FIX Scope

### Supported inbound business messages

- `35=D` NewOrderSingle
- `35=F` OrderCancelRequest
- `35=G` OrderCancelReplaceRequest

### Deferred

- persistence and recovery-aware status lookup beyond in-memory state

### Accepted tags for `35=D`

- `11` `ClOrdID`, required, must be a decimal `u64` string
- `54` `Side`, required
- `38` `OrderQty`, required
- `40` `OrdType`, required
- `44` `Price`, required for limit orders
- `55` `Symbol`, optional, informational only in v1
- `59` `TimeInForce`, optional
- `60` `TransactTime`, optional

### Accepted tags for `35=F`

- `11` `ClOrdID`, optional request identifier
- `41` `OrigClOrdID`, required, must be a decimal `u64` string
- `55` `Symbol`, optional
- `54` `Side`, optional

### Accepted tags for `35=G`

- `11` `ClOrdID`, required, replacement order id, must be a decimal `u64` string
- `41` `OrigClOrdID`, required, existing order id, must be a decimal `u64` string
- `54` `Side`, required
- `38` `OrderQty`, required
- `40` `OrdType`, required
- `44` `Price`, required for limit orders
- `55` `Symbol`, optional
- `60` `TransactTime`, optional

### Accepted tags for `35=H`

- `37` `OrderID`, preferred lookup field
- `11` `ClOrdID`, accepted as fallback lookup field
- `55` `Symbol`, optional

## Order ID Rule

There is no order-id mapping layer.

- `ClOrdID(11)` is the internal `order_id`
- `OrigClOrdID(41)` is the internal target order id for cancel
- both must be parseable as `u64`

The client is responsible for uniqueness.

## Internal Architecture

### 1. Engine core

Pure Rust library layer:

- order types
- order book
- engine state
- submit / cancel operations
- match results

This layer must not depend on Redis or FIX text parsing.

### 2. FIX adapter

Boundary layer:

- parse FIX text into domain requests
- encode domain results back into FIX text
- expose pure Rust service functions for send/query/reset flows
- accept both `|` and SOH delimiters on input
- normalize output to `|`

### 3. Redis Module host

Redis-facing layer:

- register commands
- validate arguments
- maintain global engine registry
- route to per-product engine

## State Model

Module-global state:

- `product_id -> EngineState`

Per-engine state:

- order book
- engine counters
- match history for current command result only

Optional later:

- recent execution history

## Execution Model

Single-threaded command execution in Redis thread.

Rationale:

- matches current engine design
- avoids lock-heavy coordination
- preserves deterministic ordering
- is enough for first module version

## Reply Strategy

### `FIX.SEND`

Return an array of FIX strings:

- one execution report for the accepted order state
- one execution report per trade if there are fills
- one reject message on business failure

Execution reports use deterministic `ExecID(17)` values derived from:

- order id
- execution type
- per-reply sequence number

### `FIX.BOOK`

Return a compact Redis array with:
Return an ordered Redis map with:

- `product_id`
- `best_bid`
- `best_ask`
- `bid_levels`
- `ask_levels`
- `total_bid_volume`
- `total_ask_volume`

### `FIX.STATS`

Return a compact Redis array with:

- product id
- total received orders
- matched orders
- uptime start time

## Error Policy

Protocol or parse errors:

- Redis error reply

Business rejections:

- FIX reject-style business reply as command payload

Examples:

- invalid or missing `35`
- invalid `11` or `41`
- duplicate order id
- cancel target not found
- missing `44` for limit order

## Refactor Plan

1. Add `lib.rs` and export the current engine code as a library
2. Keep the existing benchmark flow as a normal binary entry
3. Add FIX parser and encoder modules
4. Add Redis Module command layer
5. Add minimal docs and examples

## MVP Definition

The first deliverable is complete when:

- the crate builds as a library and Redis module
- `FIX.SEND` accepts `35=D`
- `FIX.SEND` accepts `35=F`
- `FIX.BOOK`, `FIX.STATS`, and `FIX.RESET` work
- replies are deterministic and easy to test
