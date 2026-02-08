// --- Message Type Constants ---

use ahash::AHashMap;
use std::collections::VecDeque;
use std::collections::{BTreeMap};
use crate::high_resolution_timer::HighResolutionTimer;

pub const MSG_ORDER_SUBMIT: u8 = 1; // Client -> Engine: Order submission
pub const MSG_ORDER_CANCEL: u8 = 2; // Client -> Engine: Order cancellation
pub const MSG_TRADE_BROADCAST: u8 = 10; // Engine -> Client: OrderExecution broadcast
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
    pub order_type: u8,   // Order side (BUY/SELL/MOCK_BUY/MOCK_SELL/) (1 byte)
    pub price_type: u8,   // Price type (LIMIT/MARKET) (1 byte)
    pub quantity: u32,    // Quantity (4 bytes)

    pub order_id: u64,    // Unique order ID (8 bytes)
    pub price: u64,       // Price (8 bytes)


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
    pub instance_tag: [u8; 16],      // 16-byte engine instance tag
    pub product_id: u16,            // Product identifier (2 bytes)
    pub bids_order_count: u32,             // Current order book size (4 bytes)
    pub ask_order_count: u32,              // Current order book size (4 bytes)
    pub matched_orders: u32,        // Total matched orders count (4 bytes)
    pub total_received_orders: u32, // Total received orders count (4 bytes)
    pub start_time: u64,            // Program start time (Nanoseconds) (8 bytes)
                                    // Total Payload Size: 42 bytes
    pub total_bid_volumn: u32,
    pub total_ask_volumn: u32,
    
}

// Match Result Structure (for MSG_TRADE_BROADCAST)
#[derive(Debug, Clone)]
pub struct OrderExecution {
    pub instance_tag: [u8; 16],    // 16-byte engine instance tag
    pub product_id: u16,          // Product identifier (2 bytes)
    pub buy_order_id: u64,        // Buyer's order ID (8 bytes)
    pub sell_order_id: u64,       // Seller's order ID (8 bytes)
    pub price: u64,               // OrderExecution price (8 bytes)
    pub quantity: u32,            // OrderExecution quantity (4 bytes)
    pub trade_time_network: u32,  // OrderExecution timestamp (Nanoseconds) (4 bytes)
    pub internal_match_time: u32, // Total Payload Size: 46 bytes
    pub is_mocked_result: bool,
}
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub order_execution_list:Vec<OrderExecution>,
    pub start_time: u64,
    pub end_time: u64,
    


}
impl MatchResult {
    pub fn new(cap: usize) -> Self {
        Self {
            order_execution_list: Vec::with_capacity(cap),
            start_time: 0,
            end_time: 0,
        }
    }
    pub fn add_order_execution(&mut self,trade: OrderExecution){
        self.order_execution_list.push(trade);
     }
     pub fn total_count(& self)->u32{
        self.order_execution_list.len() as u32
     }
     pub fn total_time(& self)-> u64{
       self.end_time - self.start_time
     }
     pub fn time_per_trade(&self)->u32{
        if self.total_count() == 0 {
            return 0
        }
        (self.total_time() / self.total_count() as u64) as u32
     }
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

#[derive(Default, Clone,Debug)]
pub struct OrdersBucket {
    pub orders: VecDeque<Order>,
}


// The core Order Book structure (T in Vec<T>)
// This implements the layered indexing (Price-Time Priority).
#[derive(Debug)]
pub struct DenseOrderBook {
    // price ladders
    pub bids: Vec<OrdersBucket>,
    pub asks: Vec<OrdersBucket>,

    // best price pointers
    pub best_bid: isize,
    pub best_ask: isize,

    // price mapping
    pub tick: u64,
    pub base_price: u64,
    pub levels: usize,

    // order_id → (is_buy, price_index)
    pub order_map: AHashMap<u64, (bool, usize)>,

    // stats
    pub total_bid_volumn: u32,
    pub total_ask_volumn: u32,

    pub match_result: MatchResult,

    pub timer: HighResolutionTimer,
}

#[derive(Debug)]
pub struct SparseOrderBook {
    // 使用 BTreeMap 自动按价格排序
    pub bids: BTreeMap<u64, OrdersBucket>,
    pub asks: BTreeMap<u64, OrdersBucket>,
    
    // order_id -> (is_buy, price) 快速索引，用于 O(log N) 取消订单
    pub order_map: AHashMap<u64, (bool, u64)>,
    
    pub total_bid_volumn: u32,
    pub total_ask_volumn: u32,
    pub match_result: MatchResult,
    
    // 基础配置（为了保持接口一致性保留）
    pub tick: u64,
    pub base_price: u64,
    
    pub timer: HighResolutionTimer,
}



pub type OrderBook = DenseOrderBook;

// Engine State and Context
#[derive(Debug)]
pub struct EngineState {
    pub instance_tag: [u8; 16],
    pub product_id: u16,
    // Order Book
    pub order_book: OrderBook,
    pub call_auction_pool:  CallAuctionPool,
    // Counters
    pub matched_orders: u64,
    pub total_received_orders: u64,
    pub start_time: u64, // Nanoseconds
}

#[derive(Debug)]
pub struct CallAuctionPool {
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
}




impl Order {
    #[inline(always)]
    pub fn is_buy(&self) -> bool {
        self.order_type == ORDER_TYPE_BUY || self.order_type == ORDER_TYPE_MOCK_BUY
    }

    #[inline(always)]
    pub fn is_sell(&self) -> bool {
        self.order_type == ORDER_TYPE_SELL || self.order_type == ORDER_TYPE_MOCK_SELL
    }

    #[inline(always)]
    pub fn is_mocked_order(&self) -> bool {
        self.order_type > 2
    }
}



