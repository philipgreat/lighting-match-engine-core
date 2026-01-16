#!/bin/bash

# 清理之前的进程
killall -9 lighting-match-engine-core 2>/dev/null

echo "开始自动化测试..."

# 第一阶段：1M 到 10M，每次1M
echo "=== 第一阶段：1M 到 10M，每次1M ==="
for i in {1..9}; do
    size="${i}M"
    echo "测试规模: $size"
    
    target/release/lighting-match-engine-core --prodid 7 --tag FIX009 --test-order-book-size $size &
    ENGINE_PID=$!
    
    sleep 20
    
    echo "进程 $ENGINE_PID 的内存使用情况:"
    pidstat -r -p $ENGINE_PID
    
    kill -9 $ENGINE_PID 2>/dev/null
    sleep 1
    echo "------------------------"
done
