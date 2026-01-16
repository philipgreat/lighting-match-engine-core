#!/bin/bash

# 清理之前的进程
killall -9 lighting-match-engine-core 2>/dev/null

echo "开始自动化测试..."

# 第一阶段：1k 到 9k，每次1k
echo "=== 第一阶段：1k 到 9k，步进1k ==="
for i in {1..9}; do
    size="${i}k"
    echo "测试规模: $size"
    
    target/release/lighting-match-engine-core --prodid 7 --tag FIX009 --test-order-book-size $size &
    ENGINE_PID=$!
    
    sleep 2
    
    echo "进程 $ENGINE_PID 的内存使用情况:"
    pidstat -r -p $ENGINE_PID
    
    kill -9 $ENGINE_PID 2>/dev/null
    sleep 1
    echo "------------------------"
done

# 第二阶段：10k 到 90k，每次10k
echo "=== 第二阶段：10k 到 90k，步进10k ==="
for i in {1..9}; do
    size="$((i*10))k"
    echo "测试规模: $size"
    
    target/release/lighting-match-engine-core --prodid 7 --tag FIX009 --test-order-book-size $size &
    ENGINE_PID=$!
    
    sleep 2
    
    echo "进程 $ENGINE_PID 的内存使用情况:"
    pidstat -r -p $ENGINE_PID
    
    kill -9 $ENGINE_PID 2>/dev/null
    sleep 1
    echo "------------------------"
done

# 第三阶段：100k 到 1000k，每次100k
echo "=== 第三阶段：100k 到 1000k，步进100k ==="
for i in {1..10}; do
    size="$((i*100))k"
    echo "测试规模: $size"
    
    target/release/lighting-match-engine-core --prodid 7 --tag FIX009 --test-order-book-size $size &
    ENGINE_PID=$!
    
    sleep 2
    
    echo "进程 $ENGINE_PID 的内存使用情况:"
    pidstat -r -p $ENGINE_PID
    
    kill -9 $ENGINE_PID 2>/dev/null
    sleep 1
    echo "------------------------"
done

echo "所有测试完成!"
