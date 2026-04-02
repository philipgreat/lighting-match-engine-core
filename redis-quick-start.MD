# Redis Quick Start

This document shows the shortest path to build the Redis module, start a local Redis server, and exercise the module with `redis-cli`.

## 1. Build the Redis module

```bash
make redis-module-release
```

Equivalent raw command:

```bash
cargo build --release --features redis-module-host
```

## 2. Start Redis with the module

Use the Make target:

```bash
make redis-module-run
```

Equivalent raw command:

```bash
/opt/homebrew/bin/redis-server --port 6380 --loadmodule /Users/Philip/githome/lighting-match-engine-core/target/release/liblighting_match_engine_core.dylib
```

The module listens on port `6380`.

## 3. Check Redis is reachable

```bash
/opt/homebrew/bin/redis-cli -p 6380 ping
```

Expected reply:

```text
PONG
```

## 4. Test commands

Submit a resting buy order:

```bash
/opt/homebrew/bin/redis-cli -p 6380 fix.send 7 '35=D|11=1001|54=1|38=5|40=2|44=101|55=AAPL|'
```

Submit a matching sell order:

```bash
/opt/homebrew/bin/redis-cli -p 6380 fix.send 7 '35=D|11=1002|54=2|38=3|40=2|44=100|55=AAPL|'
```

Typical trade reply for the incoming order above:

```text
1) "35=8|11=1002|17=1002-TRD-0|150=F|39=2|14=3|151=0|32=3|31=101|55=AAPL|"
```

Meaning:

- `150=F` means trade
- `39=2` means fully filled
- `14=3` is cumulative filled quantity
- `151=0` is leaves quantity
- `32=3` is last fill quantity
- `31=101` is last fill price

If one incoming order matches multiple resting orders, Redis returns multiple execution reports in the array. Example:

```text
1) "35=8|11=3003|17=3003-TRD-0|150=F|39=1|14=2|151=3|32=2|31=101|55=AAPL|"
2) "35=8|11=3003|17=3003-TRD-1|150=F|39=2|14=5|151=0|32=3|31=101|55=AAPL|"
```

Inspect the book:

```bash
/opt/homebrew/bin/redis-cli -p 6380 fix.book 7
```

Inspect stats:

```bash
/opt/homebrew/bin/redis-cli -p 6380 fix.stats 7
```

Replace an existing order:

```bash
/opt/homebrew/bin/redis-cli -p 6380 fix.send 7 '35=G|11=1003|41=1001|54=1|38=8|40=2|44=102|55=AAPL|'
```

Cancel an order:

```bash
/opt/homebrew/bin/redis-cli -p 6380 fix.send 7 '35=F|11=2001|41=1001|55=AAPL|'
```

Query order status:

```bash
/opt/homebrew/bin/redis-cli -p 6380 fix.send 7 '35=H|37=1001|55=AAPL|'
```

Reset module state for product `7`:

```bash
/opt/homebrew/bin/redis-cli -p 6380 fix.reset 7
```

## 5. Suggested end-to-end sequence

Run these in order:

```bash
/opt/homebrew/bin/redis-cli -p 6380 fix.reset 7
/opt/homebrew/bin/redis-cli -p 6380 fix.send 7 '35=D|11=1001|54=1|38=5|40=2|44=101|55=AAPL|'
/opt/homebrew/bin/redis-cli -p 6380 fix.send 7 '35=D|11=1002|54=2|38=3|40=2|44=100|55=AAPL|'
/opt/homebrew/bin/redis-cli -p 6380 fix.book 7
/opt/homebrew/bin/redis-cli -p 6380 fix.stats 7
/opt/homebrew/bin/redis-cli -p 6380 fix.send 7 '35=H|37=1001|55=AAPL|'
```

## 6. Stop Redis

```bash
/opt/homebrew/bin/redis-cli -p 6380 shutdown nosave
```

## 7. Docker

Build the image:

```bash
docker build -t lighting-match-engine-redis .
```

Start Redis with the module in a container:

```bash
docker run --rm -p 6379:6379 lighting-match-engine-redis
```

Test from the host:

```bash
redis-cli -p 6379 ping
redis-cli -p 6379 fix.reset 7
redis-cli -p 6379 fix.send 7 '35=D|11=1001|54=1|38=5|40=2|44=101|55=AAPL|'
redis-cli -p 6379 fix.send 7 '35=D|11=1002|54=2|38=3|40=2|44=100|55=AAPL|'
redis-cli -p 6379 fix.book 7
```

Stop the container with `Ctrl+C`, or if you started it detached:

```bash
docker ps
docker stop <container_id>
```

## Notes

- `fix.send` accepts FIX fields delimited by `|`.
- `product_id` in the examples is `7`.
- On macOS, the module file is `liblighting_match_engine_core.dylib`.
- In Docker on Linux, the module file is `liblighting_match_engine_core.so`.
- If Redis reports `does not export RedisModule_OnLoad() symbol`, rebuild with `--features redis-module-host`.
