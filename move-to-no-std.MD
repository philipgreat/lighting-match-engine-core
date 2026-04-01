# `src/` no-std 迁移检查清单

范围：

- 只检查 `src/` 目录下、且 **不包含** `src/bin/`
- 目标是判断每个 `.rs` 文件在迁移到 `no_std` / bare-metal 内核时，哪些代码可以保留，哪些必须重写，哪些应该留在宿主态

## 结论概览

建议把现有代码拆成三层：

1. `engine-core`
   - 只保留纯撮合、纯数据结构、纯编解码
   - 依赖 `core` + `alloc`
2. `host-runtime`
   - 保留 CLI、文件输出、性能报告、CPU 亲和性、宿主时钟
3. `kernel-runtime`
   - 提供内核入口、时钟抽象、输出抽象、分配器、中断/驱动

按这个标准看，当前 `src/` 下文件大致分三类：

- 可迁移但要改导入/依赖：`call_auction_pool.rs` `data_types.rs` `dense_order_book.rs` `message_codec.rs` `number_tool.rs` `sparse_order_book.rs`
- 需要明显重构后再迁移：`date_time_tool.rs` `engine_state.rs` `high_resolution_timer.rs`
- 不应该直接迁到 no-std 内核：`config.rs` `cpu_affinity.rs` `main.rs` `perf_stats.rs` `text_output_tool.rs`

---

## 逐文件检查

### `src/call_auction_pool.rs`

结论：`可迁移，但需要适配`

当前依赖：

- `std::cmp::{max, min}`
- `Vec`
- 排序、去重、克隆、`drain`

问题：

- `std::cmp` 要改成 `core::cmp`
- 这里隐含依赖堆分配；在 `no_std` 下需要显式基于 `alloc::vec::Vec`
- 当前实现本身是纯算法，没有 OS 调用，这是好事

需要重写/调整的部分：

- 把所有 `std` 导入改成 `core`/`alloc`
- 检查 `Vec`、排序、`collect()` 是否都能在 `alloc` 环境下工作
- 如果以后内核里不希望频繁分配，`critical_ticks` / `eligible_bids` / `eligible_asks` 最好改成可复用缓冲区

建议：

- 这个文件可以保留为 `engine-core` 的一部分
- 属于“低风险迁移”

### `src/config.rs`

结论：`不迁移到 no_std，保留在宿主态`

当前依赖：

- `std::env::args`
- `std::env::var`
- `String`
- `eprintln!`

问题：

- 内核没有命令行参数解析这一套宿主环境
- 内核也不应该直接依赖环境变量

需要重写的部分：

- 不建议在内核里保留这个文件
- 改成“配置结构体由宿主/引导阶段传入”或者“编译期常量配置”

建议：

- 保留在 `host-runtime`
- 内核侧新建一个 `KernelConfig`，不要直接复用这个文件

### `src/cpu_affinity.rs`

结论：`不迁移到 no_std，整体替换`

当前依赖：

- `libc`
- `windows-sys`
- `std::mem`
- Linux/macOS/Windows 平台线程 API

问题：

- 这完全是宿主 OS 的线程亲和性接口
- bare-metal 内核没有 `pthread`、没有 `thread_policy_set`、也没有 Win32 thread handle

需要重写的部分：

- 内核如果要做 CPU 绑定，需要重写成 AP 启动 / LAPIC / SMP 初始化逻辑
- 和这个文件不是同一层的概念，不能简单移植

建议：

- 删除出 `engine-core`
- 保留为宿主 benchmark 工具专用

### `src/data_types.rs`

结论：`可迁移，但需要拆分和重构`

当前依赖：

- `ahash::AHashMap`
- `std::collections::VecDeque`
- `std::collections::BTreeMap`
- `Vec`
- `HighResolutionTimer`
- trait 上有 `Send + Sync`

问题：

- 这里把“纯数据结构”和“运行时实现细节”混在了一起
- `DenseOrderBook` / `SparseOrderBook` 里直接嵌了 `HighResolutionTimer`
- `AHashMap` 当前配置是宿主态依赖；内核侧要么改 crate 配置，要么换实现
- `VecDeque` / `Vec` / `BTreeMap` 在 `no_std` 下可以用，但必须基于 `alloc`

需要重写/调整的部分：

- 所有 `std::collections::*` 改成 `alloc::collections::*`
- `Vec` 改成 `alloc::vec::Vec`
- `AHashMap` 要么：
  - 改成支持 `no_std + alloc` 的配置
  - 要么替换成 `BTreeMap`
  - 要么换 `hashbrown`
- `HighResolutionTimer` 不应出现在数据类型层，应该抽离到运行时层
- `ResultSender: Send + Sync` 需要重新评估
  - 在单核早期内核里未必需要
  - 如果保留，应改为抽象接口，不和 host 并发模型耦合

建议：

- 这是最应该优先整理的文件之一
- 最好拆成：
  - `data_types.rs`
  - `order_book_types.rs`
  - `runtime_traits.rs`

### `src/date_time_tool.rs`

结论：`需要重写`

当前依赖：

- `std::time::{SystemTime, UNIX_EPOCH}`

问题：

- bare-metal 内核不能直接用 `SystemTime`
- “当前 Unix 时间戳”在内核启动早期通常并不存在，除非你已经有 RTC、HPET、TSC 校准或外部时间源

需要重写的部分：

- 改成依赖抽象时钟接口，例如：
  - `Clock::monotonic_ns()`
  - `Clock::wall_time_ns() -> Option<u64>`

建议：

- 不要保留成独立工具文件
- 直接并入新的时钟抽象层

### `src/dense_order_book.rs`

结论：`核心可迁移，但必须做 no_std 适配`

当前依赖：

- `ahash::AHashMap`
- `vec![..]`
- `Vec`
- `VecDeque`
- `HighResolutionTimer`

问题：

- 撮合逻辑本身基本是纯算法，可以迁
- 但容器和计时器都绑了宿主态/现有实现

需要重写/调整的部分：

- 所有容器切到 `alloc`
- `AHashMap` 处理方式同 `data_types.rs`
- `timer: HighResolutionTimer` 建议移除
- `match-timing` 相关代码改成外部注入的计时器或 feature-gated hooks
- 价格越界、防止 `idx` 溢出的检查建议顺手补上

建议：

- 这是第一批应该迁进 `engine-core` 的真实业务代码
- 优先级很高

### `src/engine_state.rs`

结论：`需要明显重构后再迁移`

当前依赖：

- `std::sync::Arc`
- `std::time::{SystemTime, UNIX_EPOCH}`
- 直接构造 `OrderBook`
- 直接构造 `CallAuctionPool`

问题：

- `EngineState` 现在把：
  - 业务状态
  - 初始化策略
  - 时间获取
  - 样本订单生成
  混在一起
- 内核里不应该直接调用 `SystemTime`
- `StatusBroadcaster` 的设计明显是宿主态/网络广播思路，不适合原样进内核

需要重写/调整的部分：

- 用运行时注入时间源，替代 `SystemTime`
- `Arc` 去掉，除非未来内核真有并发共享需求
- `load_sample_test_book` / `create_buy_order` / `create_sell_order` 明显是 benchmark/helper 逻辑，应该从核心状态对象拆出去
- `StatusBroadcaster` 应移到宿主或设备层，不应留在核心引擎状态里

建议：

- `EngineState` 可以保留概念，但文件需要重写
- 只保留真正的状态聚合，不保留 host-side helper

### `src/high_resolution_timer.rs`

结论：`需要重写，不建议直接搬`

当前依赖：

- `windows-sys`
- `std::sync::OnceLock`
- `libc::clock_gettime`
- 平台条件编译
- x86 `rdtsc`
- aarch64 系统寄存器

问题：

- 这个文件虽然看起来“底层”，但本质上仍然是宿主 OS + 跨平台工具
- 在 bare-metal 内核里，你不能依赖：
  - `clock_gettime`
  - QPC
  - `OnceLock` 的当前实现路径
- TSC 校准方式也不能继续依赖宿主 OS 的 monotonic clock

需要重写的部分：

- 重新设计成内核专用时间源
- 例如：
  - x86_64: TSC + 引导阶段校准
  - 或 LAPIC/HPET/PIT 作为参考时钟
- 全局状态可改成更简单的静态初始化方案

建议：

- 不要“修修补补”这个文件
- 直接新建 `kernel_clock.rs` 更合适

### `src/main.rs`

结论：`不迁移到 no_std，整体替换`

当前依赖：

- `println!`
- `eprintln!`
- `Box<dyn std::error::Error>`
- `Vec`
- CLI 配置
- 宿主时钟
- 文件输出
- CPU affinity

问题：

- 这是用户态 benchmark/测试入口，不是内核入口
- 内核已经有独立入口，不应把这个文件继续往 `no_std` 上硬改

需要重写的部分：

- 不建议改这个文件去兼容内核
- 内核应使用独立入口
- 这个文件继续保留为 host benchmark 程序

建议：

- 保留在 `host-runtime`
- 不属于 no-std 核心迁移范围

### `src/message_codec.rs`

结论：`大部分可迁移，少量适配`

当前依赖：

- 固定字节数组
- `to_be_bytes`
- `try_into`
- `Vec<Vec<u8>>`

问题：

- 编解码大部分是纯内存操作，非常适合迁到 `core`
- 唯一明显需要处理的是 `serialize_match_result` 返回 `Vec<Vec<u8>>`

需要重写/调整的部分：

- 如果内核允许 `alloc`，这个文件基本只需要把容器显式改成 `alloc::vec::Vec`
- 如果想进一步减少分配，应改成：
  - 写入调用方提供的缓冲区
  - 或返回迭代器/分片视图

建议：

- 属于很适合放进 `engine-core` 的文件
- 优先级高

### `src/number_tool.rs`

结论：`可迁移，但建议拆成两部分`

当前依赖：

- `to_lowercase`
- `to_string`
- `String`

问题：

- `parse_human_readable_u32` 和数字分隔格式化不是一类功能
- 其中：
  - 解析函数更接近宿主配置输入
  - 格式化函数更接近输出层

需要重写/调整的部分：

- `String` 改到 `alloc::string::String`
- 如果想进一步减少分配：
  - 解析部分可以做成纯切片逻辑
  - 格式化部分可以改成写入调用方缓冲区

建议：

- `parse_human_readable_u32` 可保留在 host 配置层
- `Separatable` 更适合输出层，不属于内核核心撮合逻辑
- 不是优先迁移对象

### `src/perf_stats.rs`

结论：`不迁移到 no_std，保留在宿主态`

当前依赖：

- `std::fs::File`
- `std::io::Write`
- `SystemTime`
- `libc::localtime_r`
- `print!` / `println!`

问题：

- 文件输出和本地时间格式化都是典型宿主能力
- 百分位计算逻辑本身可迁，但这个文件整体不是内核核心组件

需要重写/调整的部分：

- 如果想复用统计逻辑，可以只抽出：
  - `Stats`
  - `calculate_perf`
- `save_perf_to_file` 和 `print_stats_table` 应留在 host 侧

建议：

- 拆分为：
  - `perf_core.rs`
  - `perf_host_io.rs`

### `src/sparse_order_book.rs`

结论：`核心可迁移，但需要 no_std 容器适配`

当前依赖：

- `ahash::AHashMap`
- `std::collections::{BTreeMap, VecDeque}`
- `Vec`
- `HighResolutionTimer`

问题：

- 业务逻辑本身仍然是纯撮合，可以迁
- 容器和计时器同样要去宿主化

需要重写/调整的部分：

- `std::collections::*` 改到 `alloc::collections::*`
- `Vec` 改为 `alloc::vec::Vec`
- `AHashMap` 重新选型
- `timer` 去掉或外置

建议：

- 可以迁，但优先级低于 `dense_order_book.rs`
- 因为当前项目默认 `OrderBook = DenseOrderBook`

### `src/text_output_tool.rs`

结论：`不迁移到 no_std，整体替换`

当前依赖：

- `println!`
- `format!`
- `String` 分配

问题：

- 这是典型宿主控制台输出模块
- bare-metal 内核的输出设备可能是：
  - VGA
  - serial
  - frame buffer
- 不能直接复用 `println!`

需要重写的部分：

- 内核侧应提供独立 writer 接口
- 如果还想保留表格展示，应该让它输出到一个 `fmt::Write` 风格的抽象，而不是直接 `println!`

建议：

- 保留 host 版
- 内核侧单独实现输出层

---

## 优先迁移顺序

建议按下面顺序推进：

1. `data_types.rs`
   - 先把纯数据结构和运行时依赖拆开
2. `dense_order_book.rs`
   - 迁移真实核心撮合逻辑
3. `message_codec.rs`
   - 迁移纯编解码
4. `call_auction_pool.rs`
   - 迁移集合竞价逻辑
5. `engine_state.rs`
   - 在前面稳定后再重构状态聚合层
6. `high_resolution_timer.rs`
   - 用新的内核时钟替代，而不是直接迁

---

## 建议保留在宿主态的文件

这些文件不建议直接迁进 no-std 内核：

- `src/main.rs`
- `src/config.rs`
- `src/cpu_affinity.rs`
- `src/perf_stats.rs`
- `src/text_output_tool.rs`

原因很简单：

- 它们解决的是“宿主程序怎么跑、怎么配、怎么打印、怎么落盘”
- 不是“撮合核心如何在 bare-metal 上执行”

---

## 建议的下一步代码动作

建议下一步不是直接乱改全部文件，而是做一个明确的代码重组：

1. 新建 `src/engine_core/`
2. 先迁：
   - `data_types`
   - `dense_order_book`
   - `message_codec`
   - `call_auction_pool`
3. 把所有 `std` 容器换成 `alloc`
4. 把计时、输出、配置、文件 IO 从核心逻辑里抽走
5. 再让：
   - `src/main.rs` 调 host runtime
   - `src/bin/kernel.rs` 调 kernel runtime

这样迁移风险最低，也最容易持续验证。

## 当前进度补充

截至当前代码状态，已经完成的拆分有：

1. `src/engine_core/`
   - 已承载：
     - `types`
     - `dense_order_book`
     - `sparse_order_book`
     - `call_auction_pool`
     - `message_codec`
     - `engine_state`
     - `clock`
2. `src/host_runtime/`
   - 已承载：
     - `perf`
     - `text_output`

这意味着当前代码已经开始形成：

- `engine_core`: 面向 `core + alloc`
- `host_runtime`: 面向宿主程序

但还没有完全清理旧文件包装层。
