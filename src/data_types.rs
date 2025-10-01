use std::sync::Arc;
use tokio::sync::Mutex;

// --- Message Type Constants ---
pub const MSG_ORDER_SUBMIT: u8 = 1;      // Client -> Engine: Order Submission
pub const MSG_ORDER_CANCEL: u8 = 2;      // Client -> Engine: Order Cancellation
pub const MSG_TRADE_BROADCAST: u8 = 10;  // Engine -> Client: Trade Broadcast
pub const MSG_STATUS_BROADCAST: u8 = 11; // Engine -> Client: Status Broadcast

// --- Order Type Constants ---
pub const ORDER_TYPE_BUY: u8 = 1;          // Order Direction: Buy
pub const ORDER_TYPE_SELL: u8 = 2;         // Order Direction: Sell
pub const ORDER_PRICE_TYPE_LIMIT: u8 = 1;  // Order Price Type: Limit
pub const ORDER_PRICE_TYPE_MARKET: u8 = 2; // Order Price Type: Market

// Unified Message Packet Size (50 bytes). Structure: [Checksum u8] [Type u8] [Payload...]
pub const MESSAGE_TOTAL_SIZE: usize = 50;

// --- Data Structures ---

// Order Structure - used for MSG_ORDER_SUBMIT
#[derive(Debug, Clone)] // Clone trait required since we removed Copy
pub struct Order {
    pub product_id: u16,   // Product ID (2 bytes)
    pub order_id: u64,   // Order ID (8 bytes)
    pub price: u64,      // Price (8 bytes)
    pub quantity: u32,   // Quantity (4 bytes)
    pub order_type: u8,  // Order Direction (1 byte, BUY/SELL)
    pub price_type: u8,  // Price Type (1 byte, LIMIT/MARKET)
    pub submit_time: u64, // Submission Timestamp (Nanoseconds from epoch) (8 bytes)
    pub expire_time: u64, // Expiry Timestamp (Nanoseconds from epoch. 0 means GTC) (8 bytes)
    // Total Payload Size: 40 bytes
}

// Order Cancellation Structure - used for MSG_ORDER_CANCEL
#[derive(Debug, Clone, Copy)]
pub struct CancelOrder {
    pub product_id: u16,  // Product ID (2 bytes)
    pub order_id: u64,  // Order ID to cancel (8 bytes)
    // Total Payload Size: 10 bytes
}

// Match Result Structure - used for MSG_TRADE_BROADCAST
#[derive(Debug, Clone, Copy)]
pub struct MatchResult {
    pub product_id: u16,      // Product ID (2 bytes)
    pub buy_order_id: u64,  // Buy Order ID (8 bytes)
    pub sell_order_id: u64, // Sell Order ID (8 bytes)
    pub price: u64,         // Trade Price (8 bytes)
    pub quantity: u32,      // Trade Quantity (4 bytes)
    pub trade_time: u64,    // Trade Timestamp (Nanoseconds from epoch) (8 bytes)
    pub instance_tag: [u8; 8], // Engine Instance Tag (8 bytes)
    // Total Payload Size: 38 + 8 = 46 bytes
}

// Broadcast Stats Structure - used for MSG_STATUS_BROADCAST
#[derive(Debug, Clone, Copy)]
pub struct BroadcastStats {
    pub product_id: u16,              // Product ID (2 bytes)
    pub order_book_size: u64,       // Order Book Size (8 bytes)
    pub matched_orders: u64,        // Matched Orders Count (8 bytes)
    pub total_received_orders: u64, // Total Received Orders Count (8 bytes)
    pub start_time: u64,            // Program Start Time (8 bytes)
    pub instance_tag: [u8; 8],        // Engine Instance Tag (8 bytes)
    // Total Payload Size: 34 + 8 = 42 bytes
}

// Unified Message Type for the Matcher
#[derive(Debug)]
pub enum IncomingMessage {
    Order(Order),
    Cancel(CancelOrder),
}

// Order book stored in EngineState
#[derive(Clone)]
pub struct EngineState {
    pub product_id: u16,       // Product ID handled by the current engine
    pub instance_tag: [u8; 8], // 8-byte tag for this engine instance
    // Order Book
    pub order_book: Arc<Mutex<Vec<Order>>>,
    // Counters
    pub matched_orders: Arc<Mutex<u64>>,
    pub total_received_orders: Arc<Mutex<u64>>,
    pub start_time: u64, // Nanoseconds
    // Broadcast Socket and Address
    pub broadcast_socket: Arc<tokio::net::UdpSocket>,
    pub multicast_addr: String,
}
