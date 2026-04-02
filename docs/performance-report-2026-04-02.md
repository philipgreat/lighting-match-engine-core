# Performance Report 2026-04-02

## Scope

This report summarizes local performance measurements for:

- the matching engine core
- the Redis Module end-to-end latency

The measurements were taken from the current repository state on 2026-04-02.

Relevant code paths:

- benchmark entry: [src/main.rs](/Users/Philip/githome/lighting-match-engine-core/src/main.rs)
- Redis reused-connection benchmark: [src/bin/redis_module_latency.rs](/Users/Philip/githome/lighting-match-engine-core/src/bin/redis_module_latency.rs)

## 1. Matching Core Benchmark

Command:

```bash
cargo run --features match-timing --release -- --prodid 7 --name AAPL --test-order-book-size 50k
```

Configuration:

- product id: `7`
- instance tag: `AAPL`
- order book preload: `50,000 bids + 50,000 asks`

Result:

- elapsed: `4,291,208 ns`
- total match results: `20,000`
- throughput: `4,660,692 match results/sec`

Latency percentiles:

- `P50 = 41 ns`
- `P90 = 83 ns`
- `P95 = 84 ns`
- `P98 = 125 ns`
- `P99 = 125 ns`
- `P999 = 316 ns`
- `P100 = 3566 ns`

Observed last result:

- total time: `125 ns`
- order executions: `5`
- average per execution: `25 ns`

Sample file:

- [perf-data/2026-04-02-01-51-48.perf](/Users/Philip/githome/lighting-match-engine-core/perf-data/2026-04-02-01-51-48.perf)

Notes:

- the core engine remains in the nanosecond range
- main latency mass is concentrated below `125 ns`
- long-tail outliers exist but are rare

## 2. Redis Module End-to-End Latency

Two measurement styles were used:

1. single-shot `redis-cli` invocation
2. reused-connection Rust benchmark

The two modes answer different questions and should not be compared without context.

### 2.1 Single-shot `redis-cli`

This path includes:

- `redis-cli` process startup
- TCP connect
- Redis command parsing
- module execution
- response output

#### `fix.stats 7`

- avg: `3.885909 ms`
- p50: `3.834963 ms`
- p95: `4.448891 ms`
- p99: `4.617929 ms`
- min: `3.458977 ms`
- max: `5.650997 ms`

#### `fix.send 7 '35=H|37=700001|55=AAPL|'`

- avg: `3.885685 ms`
- p50: `3.793001 ms`
- p95: `4.379034 ms`
- p99: `4.580975 ms`
- min: `3.412008 ms`
- max: `4.889011 ms`

#### `fix.send 7 '35=D|11=<unique>|54=1|38=1|40=2|44=101|55=AAPL|'`

- avg: `4.752089 ms`
- p50: `3.902197 ms`
- p95: `4.473925 ms`
- p99: `4.772902 ms`
- min: `3.509998 ms`
- max: `165.999889 ms`

Notes:

- this mode is dominated by client process and short-connection overhead
- it is useful for rough user-visible CLI timing, not steady-state service latency

### 2.2 Reused-Connection Rust Benchmark

Script:

- [src/bin/redis_module_latency.rs](/Users/Philip/githome/lighting-match-engine-core/src/bin/redis_module_latency.rs)

This benchmark uses:

- one persistent TCP connection
- RESP written directly from Rust
- warmup before sampling

Test parameters:

- warmup: `100`
- iterations: `1000`

#### `mode=stats`

- avg: `0.068734 ms`
- p50: `0.068959 ms`
- p95: `0.098292 ms`
- p99: `0.114916 ms`
- min: `0.020125 ms`
- max: `0.228250 ms`

#### `mode=status`

- avg: `0.069824 ms`
- p50: `0.068667 ms`
- p95: `0.101500 ms`
- p99: `0.122209 ms`
- min: `0.026042 ms`
- max: `0.213708 ms`

#### `mode=new`

- avg: `0.072669 ms`
- p50: `0.070792 ms`
- p95: `0.106791 ms`
- p99: `0.119833 ms`
- min: `0.034458 ms`
- max: `0.218000 ms`

Notes:

- with connection reuse, end-to-end Redis Module latency is around `70 us`
- `stats`, `status`, and `new` are all in the same latency band
- this is a more representative steady-state local latency figure than single-shot `redis-cli`

## 3. Comparison Summary

### Matching core

- order execution latency mainly in the `tens of ns`

### Redis Module with reused connection

- end-to-end local latency around `0.07 ms`

### Single-shot `redis-cli`

- end-to-end local latency around `4 ms`

## 4. Interpretation

The current bottleneck is not the matching core.

The main latency amplification comes from the client access pattern:

- new client process per request is expensive
- short TCP connection usage is expensive
- persistent connections reduce local end-to-end latency by roughly two orders of magnitude

For realistic service measurements, the reused-connection benchmark is the preferred baseline.

## 5. Reproduction Commands

### Matching core

```bash
cargo run --features match-timing --release -- --prodid 7 --name AAPL --test-order-book-size 50k
```

### Start Redis with module

```bash
/opt/homebrew/bin/redis-server --port 6380 --save '' --appendonly no --loadmodule /absolute/path/to/liblighting_match_engine_core.dylib
```

### Reused-connection benchmark

```bash
cargo run --release --bin redis_module_latency -- --port 6380 --mode stats
cargo run --release --bin redis_module_latency -- --port 6380 --mode status --seed-order-id 700001
cargo run --release --bin redis_module_latency -- --port 6380 --mode new --seed-order-id 800000
```

### Shutdown Redis

```bash
/opt/homebrew/bin/redis-cli -p 6380 shutdown nosave
```

## 6. Dense vs Sparse Comparison

An additional comparison was run after switching:

```rust
pub type OrderBook = SparseOrderBook;
```

The goal was to compare:

- core matching performance
- Redis Module end-to-end latency under a reused connection

### 6.1 Core benchmark comparison

DenseOrderBook baseline:

- throughput: `4,660,692 match results/sec`
- `P50 = 41 ns`
- `P99 = 125 ns`

SparseOrderBook result:

- throughput: `1,536,236 match results/sec`
- `P50 = 125 ns`
- `P90 = 183 ns`
- `P95 = 225 ns`
- `P98 = 383 ns`
- `P99 = 725 ns`
- `P999 = 4500 ns`
- `P100 = 41616 ns`

Interpretation:

- for the core matching path, the difference is not small
- SparseOrderBook is roughly `3x` slower in this benchmark
- tail latency is also materially worse

### 6.2 Redis Module reused-connection comparison

DenseOrderBook reused-connection baseline:

#### `mode=stats`

- avg: `0.068734 ms`
- p50: `0.068959 ms`
- p95: `0.098292 ms`
- p99: `0.114916 ms`

#### `mode=new`

- avg: `0.072669 ms`
- p50: `0.070792 ms`
- p95: `0.106791 ms`
- p99: `0.119833 ms`

SparseOrderBook reused-connection result:

#### `mode=stats`

- avg: `0.054561 ms`
- p50: `0.049833 ms`
- p95: `0.100750 ms`
- p99: `0.133250 ms`

#### `mode=new`

- avg: `0.058418 ms`
- p50: `0.052666 ms`
- p95: `0.101042 ms`
- p99: `0.126500 ms`

Interpretation:

- at the Redis Module end-to-end level, the difference is small
- fixed costs from RESP handling, TCP, and module dispatch compress the gap
- this matches the practical observation that client-visible local latency changes much less than the raw engine benchmark

### 6.3 Final reading

Both statements can be true at the same time:

- core matching performance differs substantially
- end-to-end local Redis Module latency differs only slightly

If the objective is ultra-low-latency matching, DenseOrderBook remains the better default.

If the objective is acceptable end-to-end module latency with sparser price coverage and lower memory footprint, SparseOrderBook remains a valid tradeoff.

## 7. Recommended Next Steps

- measure multi-connection concurrency
- measure pipelined command submission
- compare different order-book preload sizes such as `10k`, `50k`, and `100k`
