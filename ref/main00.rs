use std::net::{UdpSocket, SocketAddr};
use tokio::sync::mpsc;
use tokio::task;
use std::time::Duration;

// 定义订单和撮合结果的数据结构
const ORDER_TYPE_BUY: u8 = 1;       // 订单类型：买入
const ORDER_TYPE_SELL: u8 = 2;      // 订单类型：卖出
const ORDER_PRICE_TYPE_LIMIT: u8 = 1;  // 订单类型：限价
const ORDER_PRICE_TYPE_MARKET: u8 = 2; // 订单类型：市价

// 订单结构
#[derive(Debug, Clone)]
struct Order {
    order_id: u64,         // 订单号
    price: u64,            // 价格，单位是最小单位（如分）
    quantity: u32,         // 数量
    order_type: u8,        // 订单类型，1: buy, 2: sell
    price_type: u8,        // 价格类型，1: limit, 2: market
}

// 撮合结果结构
struct MatchResult {
    buy_order: Order,
    sell_order: Order,
    price: u64,            // 交易价格
    quantity: u32,         // 交易数量
}

// 主引擎结构
struct MatchingEngine {
    socket: UdpSocket,
    asset_id: u16,             // 资产编号
    order_book: Vec<Order>,    // 订单簿
    multicast_group: String,
    order_counter: u64,        // 用于跟踪订单包编号
    matched_orders: u64,       // 已撮合成交的订单数量
    total_received_orders: u64, // 总接收到的订单数量
    queue: mpsc::Sender<Vec<u8>>, // 消息队列发送端
    start_time: u64,           // 程序启动时间
}


impl MatchingEngine {
    // 创建新的匹配引擎实例
    fn new(asset_id: u16, multicast_group: &str) -> Self {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Couldn't bind to address");
        socket.join_multicast_v4(&multicast_group.parse().unwrap(), &"0.0.0.0".parse().unwrap())
            .expect("Couldn't join multicast group");

        MatchingEngine {
            socket,
            asset_id,
            order_book: Vec::new(),
            multicast_group: multicast_group.to_string(),
            order_counter: 0,
            matched_orders: 0,
            total_received_orders: 0,
            queue: tx,
            start_time: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
        }
    }

    // 接收并处理订单
    async fn receive_orders(&mut self, tx: mpsc::Sender<Vec<u8>>) {
        let mut buf = [0; 1400]; // 设定最大包大小为 1400 字节
        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((size, _src)) => {
                    // 将接收到的消息放入队列
                    let message = buf[0..size].to_vec();
                    if let Err(_) = tx.send(message).await {
                        eprintln!("Failed to send received message to queue.");
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving message: {}", e);
                }
            }
        }
    }

    // 处理接收到的订单
    async fn process_order(&mut self, buf: &[u8]) {
        let mut idx = 0;

        while idx + 25 <= buf.len() { // 每个消息 25 字节
            let asset_id = u16::from_be_bytes(buf[idx..idx+2].try_into().unwrap());
            let msg_type = buf[idx + 2];
            let order_id = u64::from_be_bytes(buf[idx + 3..idx + 11].try_into().unwrap());
            let price = u64::from_be_bytes(buf[idx + 11..idx + 19].try_into().unwrap());
            let quantity = u32::from_be_bytes(buf[idx + 19..idx + 23].try_into().unwrap());
            let order_type = buf[idx + 23];
            let price_type = buf[idx + 24];
            
            let order = Order {
                order_id,
                price,
                quantity,
                order_type,
                price_type,
            };

            self.order_book.push(order.clone());
            println!("Received order: {:?} (Asset ID: {}, Message Type: {})", order, asset_id, msg_type);
            
            // 简化的匹配逻辑
            if let Some(matched_order) = self.match_orders(&order) {
                self.broadcast_match(matched_order).await;
            }
            
            // Move to next message (25 bytes per message)
            idx += 40;
        }
    }

    // 撮合买卖订单
    fn match_orders(&mut self, new_order: &Order) -> Option<MatchResult> {
        for order in self.order_book.iter() {
            if new_order.order_type == ORDER_TYPE_BUY && order.order_type == ORDER_TYPE_SELL && new_order.price >= order.price {
                let quantity = new_order.quantity.min(order.quantity);
                return Some(MatchResult {
                    buy_order: new_order.clone(),
                    sell_order: order.clone(),
                    price: order.price,
                    quantity,
                });
            }
        }
        None
    }

    // 广播撮合结果
    async fn broadcast_match(&self, match_result: MatchResult) {
        let result_data = [
            match_result.buy_order.order_id,
            match_result.buy_order.price,
            match_result.buy_order.quantity as u64,
            match_result.buy_order.order_type as u64,
            match_result.buy_order.price_type as u64,
            match_result.sell_order.order_id,
            match_result.sell_order.price,
            match_result.sell_order.quantity as u64,
            match_result.sell_order.order_type as u64,
            match_result.sell_order.price_type as u64,
            match_result.price,
            match_result.quantity as u64,
        ];

        let result_bytes = unsafe { 
            std::mem::transmute::<_, [u8; 120]>(result_data) 
        };

        self.socket.send_to(&result_bytes, &self.multicast_group)
            .expect("Failed to send match result");
        println!("Broadcasting match result: {:?}", match_result);
    }

    async fn broadcast_stats(&self) {
        let stats = BroadcastStats {
            order_book_size: self.order_book.len(),
            queue_size: self.queue.capacity(),
            matched_orders: self.matched_orders,
            total_received_orders: self.total_received_orders,
            start_time: self.start_time,
        };

        let stats_bytes = serde_json::to_vec(&stats).expect("Failed to serialize stats");

        // 发送到组播地址
        self.socket.send_to(&stats_bytes, &self.multicast_group)
            .expect("Failed to send stats");
        println!("Broadcasting stats: {:?}", stats);
    }
}

#[tokio::main]
async fn main() {
    // 创建引擎实例
    let engine = MatchingEngine::new(1, "224.0.0.1:5000");

    // 创建消息队列用于缓存接收到的消息
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(100);  // 队列容量 100

    // 启动接收消息任务
    let receive_task = task::spawn(async move {
        engine.receive_orders(tx).await;
    });

    // 启动处理消息任务
    let process_task = task::spawn(async move {
        while let Some(message) = rx.recv().await {
            engine.process_order(&message).await;
        }
    });


    // 启动定期广播订单簿的任务
    let broadcast_task = task::spawn(async move {
        loop {
            engine.broadcast_order_book().await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // 等待任务完成
    receive_task.await.unwrap();
    process_task.await.unwrap();
    broadcast_task.await.unwrap();
}
