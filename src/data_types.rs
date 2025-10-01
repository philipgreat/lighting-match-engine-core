use std::sync::Arc;
use tokio::sync::Mutex;

// --- 消息类型常量 (Message Type Constants) ---
pub const MSG_ORDER_SUBMIT: u8 = 1;      // 客户端 -> 引擎：订单提交
pub const MSG_ORDER_CANCEL: u8 = 2;      // 客户端 -> 引擎：订单撤销
pub const MSG_TRADE_BROADCAST: u8 = 10;  // 引擎 -> 客户端：成交广播
pub const MSG_STATUS_BROADCAST: u8 = 11; // 引擎 -> 客户端：状态广播

// --- 订单类型常量 (Order Type Constants) ---
pub const ORDER_TYPE_BUY: u8 = 1;          // 订单方向：买入
pub const ORDER_TYPE_SELL: u8 = 2;         // 订单方向：卖出
pub const ORDER_PRICE_TYPE_LIMIT: u8 = 1;  // 订单价格类型：限价
pub const ORDER_PRICE_TYPE_MARKET: u8 = 2; // 订单价格类型：市价

// 统一消息包大小
pub const MESSAGE_TOTAL_SIZE: usize = 50;

// --- 数据结构定义 (Data Structures) ---

// 订单结构 (Order) - 用于 MSG_ORDER_SUBMIT
#[derive(Debug, Clone)] // Clone trait required since we removed Copy
pub struct Order {
    pub asset_id: u16,   // 资产编号 (2 bytes)
    pub order_id: u64,   // 订单号 (8 bytes)
    pub price: u64,      // 价格 (8 bytes)
    pub quantity: u32,   // 数量 (4 bytes)
    pub order_type: u8,  // 订单方向 (1 byte, BUY/SELL)
    pub price_type: u8,  // 价格类型 (1 byte, LIMIT/MARKET)
    pub submit_time: u64, // 下单时间戳 (8 bytes)
    pub expire_time: u64, // 过期时间戳 (8 bytes)
    // Total Payload Size: 2+8+8+4+1+1+8+8 = 40 bytes
}

// 订单撤销结构 (CancelOrder) - 用于 MSG_ORDER_CANCEL
#[derive(Debug, Clone, Copy)]
pub struct CancelOrder {
    pub asset_id: u16,  // 资产编号 (2 bytes)
    pub order_id: u64,  // 要撤销的订单号 (8 bytes)
    // Total Payload Size: 10 bytes
}

// 撮合结果结构 (MatchResult) - 用于 MSG_TRADE_BROADCAST
#[derive(Debug, Clone, Copy)]
pub struct MatchResult {
    pub asset_id: u16,      // 资产编号 (2 bytes)
    pub buy_order_id: u64,  // 买方订单号 (8 bytes)
    pub sell_order_id: u64, // 卖方订单号 (8 bytes)
    pub price: u64,         // 交易价格 (8 bytes)
    pub quantity: u32,      // 交易数量 (4 bytes)
    // Total Payload Size: 30 bytes
}

// 广播统计结构 (BroadcastStats) - 用于 MSG_STATUS_BROADCAST
#[derive(Debug, Clone, Copy)]
pub struct BroadcastStats {
    pub asset_id: u16,              // 资产编号 (2 bytes)
    pub order_book_size: u64,       // 订单簿大小 (8 bytes)
    pub matched_orders: u64,        // 已成交订单数量 (8 bytes)
    pub total_received_orders: u64, // 总接收订单数量 (8 bytes)
    pub start_time: u64,            // 程序启动时间 (8 bytes)
    // Total Payload Size: 2+8+8+8+8 = 34 bytes
}

// 传入撮合器的统一消息类型
#[derive(Debug)]
pub enum IncomingMessage {
    Order(Order),
    Cancel(CancelOrder),
}

// 订单簿存储在 EngineState 中
#[derive(Clone)]
pub struct EngineState {
    pub asset_id: u16,
    // 订单簿 (Order Book)
    pub order_book: Arc<Mutex<Vec<Order>>>,
    // 计数器 (Counters)
    pub matched_orders: Arc<Mutex<u64>>,
    pub total_received_orders: Arc<Mutex<u64>>,
    pub start_time: u64, // 纳秒
    // 广播 Socket 和地址
    pub broadcast_socket: Arc<tokio::net::UdpSocket>,
    pub multicast_addr: String,
}
