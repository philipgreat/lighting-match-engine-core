// --- Message Type Constants ---
pub const MSG_ORDER_SUBMIT: u8 = 1; // Client -> Engine: Order submission
pub const MSG_ORDER_CANCEL: u8 = 2; // Client -> Engine: Order cancellation
pub const MSG_TRADE_BROADCAST: u8 = 10; // Engine -> Client: Trade broadcast
pub const MSG_STATUS_BROADCAST: u8 = 11; // Engine -> Client: Status broadcast

// --- Order Type Constants ---
pub const ORDER_TYPE_BUY: u8 = 1; // Order side: Buy
pub const ORDER_TYPE_SELL: u8 = 2; // Order side: Sell
pub const ORDER_PRICE_TYPE_LIMIT: u8 = 1; // Order price type: Limit
pub const ORDER_PRICE_TYPE_MARKET: u8 = 2; // Order price type: Market

// --- Message Size Constant ---
pub const MESSAGE_TOTAL_SIZE: usize = 50; // All network packets are 50 bytes fixed size.

// --- Data Structure Definitions ---

// Order Structure (for MSG_ORDER_SUBMIT)
#[derive(Debug, Clone)]
pub struct Order {
    pub product_id: u16,  // Product identifier (2 bytes)
    pub order_id: u64,    // Unique order ID (8 bytes)
    pub price: u64,       // Price (8 bytes)
    pub quantity: u32,    // Quantity (4 bytes)
    pub order_type: u8,   // Order side (BUY/SELL) (1 byte)
    pub price_type: u8,   // Price type (LIMIT/MARKET) (1 byte)
    pub submit_time: u64, // Submission timestamp (Nanoseconds) (8 bytes)
    pub expire_time: u64, // Expiration timestamp (Nanoseconds. 0 means GTC) (8 bytes)
                          // Total Payload Size: 40 bytes
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
    pub instance_tag: [u8; 8],      // 8-byte engine instance tag
    pub product_id: u16,            // Product identifier (2 bytes)
    pub order_book_size: u64,       // Current order book size (8 bytes)
    pub matched_orders: u64,        // Total matched orders count (8 bytes)
    pub total_received_orders: u64, // Total received orders count (8 bytes)
    pub start_time: u64,            // Program start time (Nanoseconds) (8 bytes)
                                    // Total Payload Size: 42 bytes
}

// Match Result Structure (for MSG_TRADE_BROADCAST)
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub instance_tag: [u8; 8],    // 8-byte engine instance tag
    pub product_id: u16,          // Product identifier (2 bytes)
    pub buy_order_id: u64,        // Buyer's order ID (8 bytes)
    pub sell_order_id: u64,       // Seller's order ID (8 bytes)
    pub price: u64,               // Trade price (8 bytes)
    pub quantity: u32,            // Trade quantity (4 bytes)
    pub trade_time_network: u32,  // Trade timestamp (Nanoseconds) (8 bytes)
    pub internal_match_time: u32, // Total Payload Size: 46 bytes
}

// Enum to unify incoming messages from the network
#[derive(Debug)]
pub enum IncomingMessage {
    Order(Order),
    Cancel(CancelOrder),
}

// Engine State and Context
#[derive(Debug)]
pub struct EngineState {
    pub instance_tag: [u8; 8],
    pub product_id: u16,
    // Order Book
    pub order_book: std::sync::Arc<tokio::sync::Mutex<Vec<Order>>>,
    // Counters
    pub matched_orders: std::sync::Arc<tokio::sync::Mutex<u64>>,
    pub total_received_orders: std::sync::Arc<tokio::sync::Mutex<u64>>,
    pub start_time: u64, // Nanoseconds
    // Multicast Addresses
    pub trade_multicast_addr: std::net::SocketAddr,
    pub status_multicast_addr: std::net::SocketAddr,
}
