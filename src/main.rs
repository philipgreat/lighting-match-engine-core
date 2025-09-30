use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::net::UdpSocket as TokioUdpSocket; // 使用 tokio 的异步 Socket
use tokio::sync::{mpsc, Mutex};
use tokio::task;

// --- 常量定义 (Constants) ---
const ORDER_TYPE_BUY: u8 = 1; // 订单类型：买入
const ORDER_TYPE_SELL: u8 = 2; // 订单类型：卖出
const ORDER_PRICE_TYPE_LIMIT: u8 = 1; // 订单价格类型：限价
const ORDER_PRICE_TYPE_MARKET: u8 = 2; // 订单价格类型：市价
const ORDER_MESSAGE_SIZE: usize = 25; // 单个订单包的固定大小 (Bytes)

// --- 数据结构定义 (Data Structures) ---

// 订单结构 (Order)
#[derive(Debug, Clone)]
struct Order {
    asset_id: u16,
    order_id: u64,   // 订单号
    price: u64,      // 价格
    quantity: u32,   // 数量
    order_type: u8,  // 订单类型，1: buy, 2: sell
    price_type: u8,  // 价格类型，1: limit, 2: market
}

// 广播统计结构 (BroadcastStats)
#[derive(Debug, Clone)]
struct BroadcastStats {
    asset_id: u16,
    order_book_size: usize,     // 订单簿大小
    matched_orders: u64,        // 已成交订单数量
    total_received_orders: u64, // 总接收订单数量
    start_time: u64,            // 程序启动时间
}

// 撮合结果结构 (MatchResult)
#[derive(Debug)]
struct MatchResult {
    asset_id: u16,
    buy_order_id: u64,
    sell_order_id: u64,
    price: u64,    // 交易价格
    quantity: u32, // 交易数量
}

// 引擎核心状态 (Engine Core State)
// 此结构体包含所有需要在多个异步任务间共享和修改的状态。
// 使用 Arc<Mutex<...>> 确保并发安全。
#[derive(Clone)]
struct MatchingEngine {
    asset_id: u16,
    // 订单簿需要被 Mutex 保护
    order_book: Arc<Mutex<Vec<Order>>>,
    // 计数器需要被 Mutex 保护
    matched_orders: Arc<Mutex<u64>>,
    total_received_orders: Arc<Mutex<u64>>,
    start_time: u64,
}

impl MatchingEngine {
    // 创建新的匹配引擎实例
    fn new(asset_id: u16) -> Self {
        MatchingEngine {
            asset_id,
            // 初始化 Arc<Mutex>
            order_book: Arc::new(Mutex::new(Vec::new())),
            matched_orders: Arc::new(Mutex::new(0)),
            total_received_orders: Arc::new(Mutex::new(0)),
            start_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    // --- 订单接收/解析逻辑 (Order Receiving/Parsing) ---

    // 接收并解析订单数据包，将订单通过 mpsc::Sender 发送给处理任务
    async fn receive_orders(
        &self,
        socket: Arc<TokioUdpSocket>, // 接收任务独占 Socket 句柄
        tx: mpsc::Sender<Order>,
    ) {
        let mut buf = [0; 1400];
        loop {
            // 使用 tokio::UdpSocket::recv_from 是非阻塞且与 tokio 运行时集成的
            match socket.recv_from(&mut buf).await {
                Ok((size, _src)) => {
                    // 接收成功，更新总接收订单数
                    // 修复: self.total_received_orders.lock().await 已经返回 MutexGuard
                    let mut total = self.total_received_orders.lock().await;
                    *total += 1;

                    // 解析并发送订单
                    if let Err(e) = self.process_order_buffer(&buf[0..size], &tx).await {
                        eprintln!("Error processing order buffer: {}", e);
                    }
                }
                // 在 tokio 中，WouldBlock 已经被运行时隐藏，这里只处理真正的错误
                Err(e) => {
                    eprintln!("Error receiving message: {}", e);
                    // 遇到错误短暂等待避免无限循环
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    // 处理接收到的缓冲区，解析出订单
    async fn process_order_buffer(
        &self,
        buf: &[u8],
        tx: &mpsc::Sender<Order>,
    ) -> Result<(), String> {
        let mut idx = 0;

        while idx + ORDER_MESSAGE_SIZE <= buf.len() {
            // 确保资产 ID 匹配 (可选的业务逻辑检查)
            let asset_id = u16::from_be_bytes(
                buf[idx..idx + 2]
                    .try_into()
                    .map_err(|_| "Invalid asset_id slice".to_string())?,
            );

            if asset_id != self.asset_id {
                idx += ORDER_MESSAGE_SIZE;
                continue; // 忽略不属于本引擎的订单
            }

            // 解析各个字段
            let _msg_type = buf[idx + 2]; // 消息类型 (未使用，保留)
            let order_id = u64::from_be_bytes(
                buf[idx + 3..idx + 11]
                    .try_into()
                    .map_err(|_| "Invalid order_id slice".to_string())?,
            );
            let price = u64::from_be_bytes(
                buf[idx + 11..idx + 19]
                    .try_into()
                    .map_err(|_| "Invalid price slice".to_string())?,
            );
            let quantity = u32::from_be_bytes(
                buf[idx + 19..idx + 23]
                    .try_into()
                    .map_err(|_| "Invalid quantity slice".to_string())?,
            );
            let order_type = buf[idx + 23];
            let price_type = buf[idx + 24];
            let asset_id=self.asset_id;
            let order = Order {
                asset_id,
                order_id,
                price,
                quantity,
                order_type,
                price_type,
            };

            // 发送订单到处理队列
            if let Err(e) = tx.send(order).await {
                eprintln!("Failed to send order to processing queue: {:?}", e);
            }
            
            // 移动到下一个消息
            idx += ORDER_MESSAGE_SIZE;
        }

        Ok(())
    }

    // --- 撮合逻辑 (Matching Logic) ---

    // 尝试撮合新订单 against 订单簿
    // 注意：这个函数现在需要锁住订单簿
    async fn match_orders(&self, new_order: &mut Order) -> Option<MatchResult> {
        let mut order_book = self.order_book.lock().await;

        let matching_index = order_book.iter().enumerate().find(|(_i, existing_order)| {
            // 简单的撮合条件：买入 vs 卖出 且 买价 >= 卖价 (Limit Order)
            // 市价单逻辑可以更复杂，这里仅处理限价撮合
            if new_order.order_type == ORDER_TYPE_BUY
                && existing_order.order_type == ORDER_TYPE_SELL
                && new_order.price >= existing_order.price
            {
                return true;
            }
            false
        });

        if let Some((i, existing_order)) = matching_index {
            let fill_quantity = new_order.quantity.min(existing_order.quantity);
            let match_price = existing_order.price;

            // 构建撮合结果
            let result = MatchResult {
                asset_id: self.asset_id,
                buy_order_id: if new_order.order_type == ORDER_TYPE_BUY {
                    new_order.order_id
                } else {
                    existing_order.order_id
                },
                sell_order_id: if new_order.order_type == ORDER_TYPE_SELL {
                    new_order.order_id
                } else {
                    existing_order.order_id
                },
                price: match_price,
                quantity: fill_quantity,
            };

            // 处理新订单的剩余数量
            new_order.quantity -= fill_quantity;

            // 处理订单簿中订单的剩余数量
            if existing_order.quantity > fill_quantity {
                // 部分成交：更新订单簿中的订单数量
                order_book[i].quantity -= fill_quantity;
            } else {
                // 完全成交：从订单簿中移除该订单
                order_book.remove(i);
            }

            // 更新撮合计数器
            // 修复: self.matched_orders.lock().await 已经返回 MutexGuard
            let mut matched_count = self.matched_orders.lock().await;
            *matched_count += 1;
            
            Some(result)
        } else {
            None
        }
    }

    // 撮合循环：从接收队列中取出订单并进行撮合
    async fn process_orders(&self, mut rx: mpsc::Receiver<Order>) {
        while let Some(mut order) = rx.recv().await {
            // 循环尝试撮合，直到订单完全成交或找不到匹配项
            while order.quantity > 0 {
                if let Some(match_result) = self.match_orders(&mut order).await {
                    self.broadcast_match(match_result).await;
                } else {
                    break; // 找不到匹配项，退出循环
                }
            }

            // 如果订单仍有剩余数量，将其加入订单簿
            if order.quantity > 0 {
                println!(
                    "Order partially/unfilled. Adding to book: {:?}",
                    order
                );
                self.add_order_to_book(order).await;
            }
        }
    }

    // 将未完全成交的订单加入订单簿
    async fn add_order_to_book(&self, order: Order) {
        let mut order_book = self.order_book.lock().await;
        order_book.push(order);
    }

    // 广播撮合结果
    async fn broadcast_match(&self, match_result: MatchResult) {
        println!("Broadcasting match: {:?}", match_result);
        // 实际实现中这里应该序列化并发送数据到 multicast_group
    }

    // 广播统计信息
    async fn broadcast_stats(&self) {
        // 锁定所有共享状态以获取一致的快照
        let order_book = self.order_book.lock().await;
        let matched_orders = self.matched_orders.lock().await;
        let total_received_orders = self.total_received_orders.lock().await;

        let stats = BroadcastStats {
            asset_id: self.asset_id,
            order_book_size: order_book.len(),
            matched_orders: *matched_orders,
            total_received_orders: *total_received_orders,
            start_time: self.start_time,
        };

        println!("Broadcasting stats: {:?}", stats);
        // 实际实现中这里应该序列化并发送数据
    }

    // --- 启动函数 (Start) ---

    // 启动引擎的各个并发任务
    pub async fn start(self, multicast_group: &str) -> std::io::Result<()> {
        let (tx, rx) = mpsc::channel::<Order>(1000); // 增大通道容量
        let core_arc = Arc::new(self);

        // 1. 设置异步 UDP Socket
        let (ip_str, port_str) = multicast_group
            .split_once(':')
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid multicast group format")
            })?;
        let port: u16 = port_str.parse().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Invalid port: {}", e))
        })?;
        let multicast_addr: Ipv4Addr = ip_str.parse().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Invalid IP: {}", e))
        })?;

        // 绑定到本地端口以接收多播消息
        let socket = TokioUdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
        socket.join_multicast_v4(multicast_addr, Ipv4Addr::new(0, 0, 0, 0))?;
        
        // 使用 Arc 包装 Socket，用于共享句柄给接收任务
        let socket_arc = Arc::new(socket); 

        // 2. 启动接收订单任务 (Receiver Task)
        let receive_core = core_arc.clone();
        let receive_task = task::spawn(async move {
            receive_core.receive_orders(socket_arc, tx).await;
        });

        // 3. 启动处理订单任务 (Processor Task)
        let process_core = core_arc.clone();
        let process_task = task::spawn(async move {
            process_core.process_orders(rx).await;
        });

        // 4. 启动广播统计任务 (Stats Task)
        let stats_core = core_arc.clone();
        let stats_task = task::spawn(async move {
            loop {
                stats_core.broadcast_stats().await;
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });

        // 等待任务完成
        tokio::select! {
            _ = receive_task => println!("Receive task finished."),
            _ = process_task => println!("Process task finished."),
            _ = stats_task => println!("Stats task finished."),
        }
        
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    // 实例化核心引擎
    let engine = MatchingEngine::new(1);
    let multicast_addr = "224.0.0.1:5000";

    println!("Starting matching engine for Asset ID 1 on {}...", multicast_addr);

    // 启动引擎
    if let Err(e) = engine.start(multicast_addr).await {
        eprintln!("Failed to start matching engine: {}", e);
    }
}
