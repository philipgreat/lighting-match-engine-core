# 🔥 Lighting Match Engine Core 🔥

**Built with Rust for Blazing-Fast Performance**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70.0-orange.svg)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/travis/com/philipgreat/lighting-match-engine-core.svg)](https://travis-ci.com/philipgreat/lighting-match-engine-core)

The Lighting Match Engine Core is a minimal, lighting-fast matching engine designed for a single purpose: **matching orders with extreme speed and reliability**.

It's a focused, no-frills engine that you can build upon. Each instance serves a single product, making it highly efficient and scalable.

## 🚀 Why Choose Lighting Match Engine Core?

*   **⚡️ Blazing Fast:** Written in Rust, it's designed for performance. We're talking nanosecond-level precision.
*   **💪 Reliable:** With minimal dependencies (only Tokio for networking), the engine is incredibly stable.
*   **💡 Simple & Focused:** It does one thing and does it well: matching. No unnecessary features, no bloat.
*   **🌐 Universal:** Use it for a wide range of products:
    *   Stocks & Cryptocurrencies
    *   Futures & Options
    *   Forex & Commodities
    *   NFTs & Real Estate
    *   ...and much more!

## ✨ Key Features

*   **Order Types:** Market and Limit orders.
*   **Matching Policies:** Price-first, time-first.
*   **High Precision:** Time is measured in nanoseconds.
*   **Lean & Mean:** 64-byte package size for efficient network communication.
*   **In-Memory Processing:** All operations happen in memory for maximum speed.
*   **UDP Multicast:** Orders are received via UDP multicast for low-latency communication.

## 🛠️ Quick Start

Get up and running in minutes!

1.  **Start the Engine:**

    ```bash
    make
    ```

    or

    ```bash
    cargo run --release -- --prodid 7 --tag FIX009 --test-order-book-size 10k
    ```

    This command starts an engine instance for product `7` with the tag `FIX009` and a test order book of 10,000 buy and sell orders.

2.  **See the Magic:**

  You'll see a match result like this:

![test screen shot ](docs/test-screen-shot.png)

  That's an internal match time(core-matching latency) of just **46 nanoseconds** per execution with 10K asks and bids respectively on An Apple M1 Max Macbook Pro.

  Code snippet in main.rs
  
```rust
    let start = timer.ns() as u64;
    
    
    for i in 0..count {

        let  new_order_buy = Order{
            product_id: 7 ,
            order_type: ORDER_TYPE_BUY,
            price:100000000000,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            quantity:5,
            order_id: 1_000_000_000+i,
            submit_time:100,
            expire_time:0,
            

        };
        
        engine_state.match_order(new_order_buy);

        let new_order_sell = Order{
            product_id: 7 ,
            order_type: ORDER_TYPE_SELL,
            price:1,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            quantity:9,
            order_id: 2_000_000_000+i+1,
            submit_time:2_000_000_000+i+1,
            expire_time:0,

        };
        engine_state.match_order(new_order_sell);

    }
    let end = timer.ns() as u64;
    println!("Time consumed {} ns for {} match request.", (end-start),2*count);
    println!("Speed: {} match results per second.", ( (1_000_000_000)*(2*count ) ) /(end-start));

```

## Redis Module Mode

The project can also run as a Redis Module host for remote invocation through Redis commands.

Performance report:

- [docs/performance-report-2026-04-02.md](/Users/Philip/githome/lighting-match-engine-core/docs/performance-report-2026-04-02.md)

### Build the module

```bash
cargo build --features redis-module-host
```

On macOS the module artifact is:

```bash
target/debug/liblighting_match_engine_core.dylib
```

On Linux the module artifact is:

```bash
target/debug/liblighting_match_engine_core.so
```

### Start Redis with the module

```bash
redis-server --loadmodule /absolute/path/to/liblighting_match_engine_core.dylib
```

### Supported Redis commands

- `fix.send <product_id> <fix_message>`
- `fix.book <product_id>`
- `fix.stats <product_id>`
- `fix.reset <product_id>`

### Supported FIX business messages

- `35=D` NewOrderSingle
- `35=F` OrderCancelRequest
- `35=G` OrderCancelReplaceRequest
- `35=H` OrderStatusRequest

This module does not implement FIX session management. FIX is only used as the business payload format carried inside Redis commands.

### FIX input rules

- `ClOrdID(11)` must be a decimal `u64` string
- `OrigClOrdID(41)` must be a decimal `u64` string
- the external caller is responsible for uniqueness
- input accepts either `|` or SOH as field delimiters
- output is normalized to `|`

### Example: submit a resting buy order

```bash
redis-cli fix.send 7 '35=D|11=1001|54=1|38=5|40=2|44=101|55=AAPL|'
```

Example reply:

```text
35=8|11=1001|17=1001-NEW-0|150=0|39=0|14=0|151=5|32=0|31=101|55=AAPL|
```

### Example: cancel an order

```bash
redis-cli fix.send 7 '35=F|11=2001|41=1001|55=AAPL|'
```

### Example: replace an order

```bash
redis-cli fix.send 7 '35=G|11=1002|41=1001|54=1|38=8|40=2|44=102|55=AAPL|'
```

### Example: inspect state

```bash
redis-cli fix.book 7
redis-cli fix.stats 7
redis-cli fix.reset 7
```

`fix.book` now returns a structured map with:

- `product_id`
- `best_bid`
- `best_ask`
- `bid_levels`
- `ask_levels`
- `total_bid_volume`
- `total_ask_volume`

### Example: query order status

```bash
redis-cli fix.send 7 '35=H|37=1002|55=AAPL|'
```

Example reply:

```text
35=8|11=1002|17=1002-CXL-0|150=4|39=4|14=0|151=8|32=0|31=102|55=AAPL|
```

### Testing

Core and FIX adapter tests:

```bash
cargo test
```

Include Redis Module feature-gated code in compilation:

```bash
cargo test --lib --features redis-module-host
```

Current automated coverage includes:

- FIX parsing for `35=D` and `35=G`
- FIX parsing for `35=H`
- duplicate order rejection
- crossing trade generation
- cancel and replace flows
- multi-fill execution report sequencing
- service-level `book`, `stats`, and `reset`
- status-query responses for known and unknown orders

If you only want to verify the module code compiles, use:

```bash
cargo check --features redis-module-host
```

Manual end-to-end verification against a local Redis instance:

```bash
redis-server --port 6380 --loadmodule /absolute/path/to/liblighting_match_engine_core.dylib
redis-cli -p 6380 fix.send 7 '35=D|11=1001|54=1|38=5|40=2|44=101|55=AAPL|'
redis-cli -p 6380 fix.send 7 '35=G|11=1002|41=1001|54=1|38=8|40=2|44=102|55=AAPL|'
redis-cli -p 6380 fix.book 7
redis-cli -p 6380 shutdown nosave
```

### Reused-Connection Latency Script

To avoid `redis-cli` process startup cost, use the persistent-connection Rust benchmark:

```bash
cargo run --release --bin redis_module_latency -- --host 127.0.0.1 --port 6380 --mode stats
```

Supported modes:

- `stats`
- `status`
- `new`

Example:

```bash
cargo run --release --bin redis_module_latency -- --port 6380 --mode new --iterations 5000 --warmup 500
```


## ⚙️ How It Works

The engine follows a simple, robust workflow:

1.  **Rebuild Order Book:** The order book is rebuilt from an order book fuel server (not included in this project).
2.  **Receive Orders:** The engine listens for incoming order requests via UDP.
3.  **Match Orders:** The core matching logic is executed.
4.  **Broadcast Results:** Matching results are broadcast to the network.

## 🧩 What's in the Box (and What's Not)

This engine is the core of a trading system. You'll need to build the surrounding systems to create a complete solution.

**In Scope:**

*   A simple, robust, and fast matching engine.

**Out of Scope:**

*   Product Management System
*   Market Data System
*   Order Management System (OMS)
*   Risk Management System
*   ...and other external systems.

## 🤝 Contributing

We welcome contributions from the community! Whether you want to fix a bug, add a feature, or improve the documentation, we'd love to have your help.

## 📜 License

This project is licensed under the [MIT License](./LICENSE.md).

## 💬 Contact

Have questions or want to get involved?

*   **Telegram:** <https://t.me/philip_is_online>
