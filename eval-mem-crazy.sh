#!/bin/bash

# 清理之前的进程
killall -9 lighting-match-engine-core 2>/dev/null

echo "开始自动化测试..."

# 第一阶段：100M 到 500M，每次100M
echo "=== 第一阶段：100M 到 500M，每次100M ==="
for i in {1..5}; do
    size="$((i*100))M"
    echo "测试规模: $size"
    
    # 启动引擎，使用静默模式避免输出干扰
    target/release/lighting-match-engine-core --prodid 7 --tag FIX009 --test-order-book-size $size 2>/dev/null &
    ENGINE_PID=$!
    
    echo "引擎启动，PID: $ENGINE_PID"
    wait_time=$((i*60)) 
    # 等待更长时间让内存稳定
    sleep ${wait_time}
    
    echo "进程 $ENGINE_PID 的内存使用情况:"
    # 多次采样获取更准确的内存数据
    for j in {1..3}; do
        echo "第 $j 次采样:"
        pidstat -r -p $ENGINE_PID 1 1
        sleep 2
    done
    
    kill -9 $ENGINE_PID 2>/dev/null
    echo "进程 $ENGINE_PID 已停止"
    sleep 3
    echo "------------------------"
    echo ""
done

echo "所有测试完成!"
