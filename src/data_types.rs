// --- Message Type Constants ---

use std::sync::Arc;
use tokio::sync::RwLock;

pub const MSG_ORDER_SUBMIT: u8 = 1; // Client -> Engine: Order submission
pub const MSG_ORDER_CANCEL: u8 = 2; // Client -> Engine: Order cancellation
pub const MSG_TRADE_BROADCAST: u8 = 10; // Engine -> Client: Trade broadcast
pub const MSG_STATUS_BROADCAST: u8 = 11; // Engine -> Client: Status broadcast

// --- Order Type Constants ---
pub const ORDER_TYPE_BUY: u8 = 1; // Order side: Buy
pub const ORDER_TYPE_SELL: u8 = 2; // Order side: Sell

pub const ORDER_TYPE_MOCK_BUY: u8 = 3; // Order side: mock buy
pub const ORDER_TYPE_MOCK_SELL: u8 = 4; // Order side: mock sell



pub const ORDER_PRICE_TYPE_LIMIT: u8 = 1; // Order price type: Limit
pub const ORDER_PRICE_TYPE_MARKET: u8 = 2; // Order price type: Market

pub const TRADE_TYPE_REAL: u8 = 0; // Order price type: Limit
pub const TRADE_TYPE_MOCK: u8 = 1; // Order price type: Market



// --- Message Size Constant ---
pub const MESSAGE_TOTAL_SIZE: usize = 64; // All network packets are 64 bytes fixed size.


// --- Data Structure Definitions ---

// Order Structure (for MSG_ORDER_SUBMIT)
#[derive(Debug, Clone)]
pub struct Order {
    pub product_id: u16,  // Product identifier (2 bytes)
    pub order_id: u64,    // Unique order ID (8 bytes)
    pub price: u64,       // Price (8 bytes)
    pub quantity: u32,    // Quantity (4 bytes)
    pub order_type: u8,   // Order side (BUY/SELL/MOCK_BUY/MOCK_SELL/) (1 byte)
    pub price_type: u8,   // Price type (LIMIT/MARKET) (1 byte)
    pub submit_time: u64, // Submission timestamp (Nanoseconds) (8 bytes)
    pub expire_time: u64, // Expiration timestamp (Nanoseconds. 0 means GTC) (8 bytes)
                          // Total Payload Size: 40 bytes
    pub is_mocked_order: bool, 
}

// Order Cancellation Structure (for MSG_ORDER_CANCEL)
#[derive(Debug, Clone)]
pub struct CancelOrder {
    pub product_id: u16, // Product identifier (2 bytes)
    pub order_id: u64,   // Order ID to cancel (8 bytes)
                         // Total Payload Size: 10 bytes
    
}

// Broadcast Status Structure (for MSG_STATUS_BROADCAST)
#[derive(Debug, Clone)]
pub struct BroadcastStats {
    pub instance_tag: [u8; 16],      // 8-byte engine instance tag
    pub product_id: u16,            // Product identifier (2 bytes)
    pub bids_size: u32,             // Current order book size (4 bytes)
    pub ask_size: u32,              // Current order book size (4 bytes)
    pub matched_orders: u32,        // Total matched orders count (4 bytes)
    pub total_received_orders: u32, // Total received orders count (4 bytes)
    pub start_time: u64,            // Program start time (Nanoseconds) (8 bytes)
                                    // Total Payload Size: 42 bytes
}

// Match Result Structure (for MSG_TRADE_BROADCAST)
#[derive(Debug, Clone)]
pub struct Trade {
    pub instance_tag: [u8; 16],    // 16-byte engine instance tag
    pub product_id: u16,          // Product identifier (2 bytes)
    pub buy_order_id: u64,        // Buyer's order ID (8 bytes)
    pub sell_order_id: u64,       // Seller's order ID (8 bytes)
    pub price: u64,               // Trade price (8 bytes)
    pub quantity: u32,            // Trade quantity (4 bytes)
    pub trade_time_network: u32,  // Trade timestamp (Nanoseconds) (4 bytes)
    pub internal_match_time: u32, // Total Payload Size: 46 bytes
    pub is_mocked_result: bool,
}
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub trade_list:Vec<Trade>,
    pub start_time: u64,
    pub end_time: u64,
    


}

// Enum to unify incoming messages from the network
#[derive(Debug)]
pub enum IncomingMessage {
    Order(Order),
    Cancel(CancelOrder),
}

// Type alias for indexing into the main orders Vec.
// u32 is used to maximize CPU cache density for indexing, covering up to 4.2 billion orders.
pub type OrderIndex = u32;

pub trait ResultSender: Send + Sync {
    fn send_result(&self, result: MatchResult);
}

#[derive(Debug, Clone, Copy)]
pub struct MatchedRestingOrder {
    pub order_index: OrderIndex, // Index in the bids or asks vector
    pub matched_quantity: u32,   // Quantity matched from this resting order
    pub is_buy: bool,            // true if the order is from the bids array (buy side)
}

#[derive(Debug)]
// The core Order Book structure (T in Vec<T>)
// This implements the layered indexing (Price-Time Priority).

pub struct OrderBook {
    // Vectors to hold the actual orders. Bids: best to worst. Asks: best to worst.
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,

    // Vectors to hold the indices of the best orders.
    pub top_bids_index: Vec<OrderIndex>,
    pub top_asks_index: Vec<OrderIndex>,

    // Configuration
    pub init_order_book_size: u32,
    pub init_top_index_size: u32,

    pub bids_index_used: usize,
    pub asks_index_used: usize,

    pub matched_orders: Vec<MatchedRestingOrder>,
    pub bids_to_remove: Vec<OrderIndex>,
    pub asks_to_remove: Vec<OrderIndex>,
    pub match_result:MatchResult,
}

// Engine State and Context
#[derive(Debug)]
pub struct EngineState {
    pub instance_tag: [u8; 16],
    pub product_id: u16,
    // Order Book
    pub order_book: Arc<RwLock<OrderBook>>,
    // Counters
    pub matched_orders: std::sync::Arc<RwLock<u64>>,
    pub total_received_orders: std::sync::Arc<RwLock<u64>>,
    pub start_time: u64, // Nanoseconds
    pub status_multicast_addr: std::net::SocketAddr,
}
