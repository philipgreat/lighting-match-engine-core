use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

pub const MSG_ORDER_SUBMIT: u8 = 1;
pub const MSG_ORDER_CANCEL: u8 = 2;
pub const MSG_TRADE_BROADCAST: u8 = 10;
pub const MSG_STATUS_BROADCAST: u8 = 11;

pub const ORDER_TYPE_BUY: u8 = 1;
pub const ORDER_TYPE_SELL: u8 = 2;

pub const ORDER_TYPE_MOCK_BUY: u8 = 3;
pub const ORDER_TYPE_MOCK_SELL: u8 = 4;

pub const ORDER_PRICE_TYPE_LIMIT: u8 = 1;
pub const ORDER_PRICE_TYPE_MARKET: u8 = 2;

pub const TRADE_TYPE_REAL: u8 = 0;
pub const TRADE_TYPE_MOCK: u8 = 1;

pub const MESSAGE_TOTAL_SIZE: usize = 64;

#[repr(C, align(64))]
#[derive(Debug, Clone)]
pub struct Order {
    pub product_id: u16,
    pub order_side: u8,
    pub price_type: u8,
    pub quantity: u32,
    pub order_id: u64,
    pub price: u64,
    pub submit_time: u64,
    pub expire_time: u64,
    pub _padding: [u8; 24],
}

#[derive(Debug, Clone)]
pub struct CancelOrder {
    pub product_id: u16,
    pub order_id: u64,
}

#[derive(Debug, Clone)]
pub struct BroadcastStats {
    pub instance_tag: [u8; 16],
    pub product_id: u16,
    pub bids_order_count: u32,
    pub ask_order_count: u32,
    pub matched_orders: u32,
    pub total_received_orders: u32,
    pub start_time: u64,
    pub total_bid_volumn: u32,
    pub total_ask_volumn: u32,
}

#[derive(Debug, Clone)]
pub struct OrderExecution {
    pub instance_tag: [u8; 16],
    pub product_id: u16,
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub price: u64,
    pub quantity: u32,
    pub trade_time_network: u32,
    pub internal_match_time: u32,
    pub is_mocked_result: bool,
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub order_execution_list: Vec<OrderExecution>,
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

    pub fn add_order_execution(&mut self, trade: OrderExecution) {
        self.order_execution_list.push(trade);
    }

    pub fn total_count(&self) -> u32 {
        self.order_execution_list.len() as u32
    }

    pub fn total_time(&self) -> u64 {
        self.end_time - self.start_time
    }

    pub fn time_per_order_execution(&self) -> u32 {
        if self.total_count() == 0 {
            return 0;
        }
        (self.total_time() / self.total_count() as u64) as u32
    }
}

#[derive(Debug)]
pub enum IncomingMessage {
    Order(Order),
    Cancel(CancelOrder),
}

pub type OrderIndex = u32;

pub trait ResultSender: Send + Sync {
    fn send_result(&self, result: MatchResult);
}

#[derive(Debug, Clone, Copy)]
pub struct MatchedRestingOrder {
    pub order_index: OrderIndex,
    pub matched_quantity: u32,
    pub is_buy: bool,
}

#[derive(Default, Clone, Debug)]
pub struct OrdersBucket {
    pub orders: VecDeque<Order>,
}

#[derive(Debug)]
pub struct DenseOrderBook {
    pub bids: Vec<OrdersBucket>,
    pub asks: Vec<OrdersBucket>,
    pub best_bid: isize,
    pub best_ask: isize,
    pub tick: u64,
    pub base_price: u64,
    pub levels: usize,
    pub order_map: BTreeMap<u64, (bool, usize)>,
    pub total_bid_volume: u32,
    pub total_ask_volume: u32,
    pub match_result: MatchResult,
}

#[derive(Debug)]
pub struct SparseOrderBook {
    pub bids: BTreeMap<u64, OrdersBucket>,
    pub asks: BTreeMap<u64, OrdersBucket>,
    pub order_map: BTreeMap<u64, (bool, u64)>,
    pub total_bid_volume: u32,
    pub total_ask_volume: u32,
    pub match_result: MatchResult,
    pub tick: u64,
    pub base_price: u64,
}

#[derive(Debug)]
pub struct CallAuctionPool {
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
}

#[derive(Debug)]
pub struct EngineState {
    pub instance_tag: [u8; 16],
    pub product_id: u16,
    pub order_book: DenseOrderBook,
    pub call_auction_pool: CallAuctionPool,
    pub matched_orders: u64,
    pub total_received_orders: u64,
    pub start_time: u64,
}

impl Order {
    #[inline(always)]
    pub fn is_buy(&self) -> bool {
        self.order_side == ORDER_TYPE_BUY || self.order_side == ORDER_TYPE_MOCK_BUY
    }

    #[inline(always)]
    pub fn is_sell(&self) -> bool {
        self.order_side == ORDER_TYPE_SELL || self.order_side == ORDER_TYPE_MOCK_SELL
    }

    #[inline(always)]
    pub fn is_mocked_order(&self) -> bool {
        self.order_side > 2
    }
}
