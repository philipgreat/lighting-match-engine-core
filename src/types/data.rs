use ahash::AHashMap;
use std::error::Error;
use std::fmt;
use std::collections::BTreeMap;
use std::collections::VecDeque;

use crate::timer::HighResolutionTimer;

pub const MSG_ORDER_SUBMIT: u8 = 1;
pub const MSG_ORDER_CANCEL: u8 = 2;
pub const MSG_TRADE_BROADCAST: u8 = 10;
pub const MSG_STATUS_BROADCAST: u8 = 11;
pub const MSG_ERROR_REPLY: u8 = 12;

pub const TRADE_TYPE_REAL: u8 = 0;
pub const TRADE_TYPE_MOCK: u8 = 1;

pub const MESSAGE_TOTAL_SIZE: usize = 64;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy = 1,
    Sell = 2,
}

impl OrderSide {
    pub fn to_wire(self, is_mock: bool) -> u8 {
        match (self, is_mock) {
            (Self::Buy, false) => 1,
            (Self::Sell, false) => 2,
            (Self::Buy, true) => 3,
            (Self::Sell, true) => 4,
        }
    }

    pub fn from_wire(value: u8) -> Result<(Self, bool), &'static str> {
        match value {
            1 => Ok((Self::Buy, false)),
            2 => Ok((Self::Sell, false)),
            3 => Ok((Self::Buy, true)),
            4 => Ok((Self::Sell, true)),
            _ => Err("Invalid order side"),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceType {
    Limit = 1,
    Market = 2,
}

impl PriceType {
    pub fn to_wire(self) -> u8 {
        self as u8
    }

    pub fn from_wire(value: u8) -> Result<Self, &'static str> {
        match value {
            1 => Ok(Self::Limit),
            2 => Ok(Self::Market),
            _ => Err("Invalid price type"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OrderFlags {
    pub is_mock: bool,
}

#[repr(C, align(64))]
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub product_id: u16,
    pub side: OrderSide,
    pub price_type: PriceType,
    pub flags: OrderFlags,
    pub quantity: u32,
    pub order_id: u64,
    pub price: u64,
    pub submit_time: u64,
    pub expire_time: u64,
    pub _padding: [u8; 24],
}

#[derive(Debug, Clone)]
pub struct RestingOrder {
    pub product_id: u16,
    pub order_id: u64,
    pub side: OrderSide,
    pub price_type: PriceType,
    pub flags: OrderFlags,
    pub price: u64,
    pub original_quantity: u32,
    pub remaining_quantity: u32,
    pub submit_time: u64,
    pub expire_time: u64,
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
pub struct Trade {
    pub product_id: u16,
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub price: u64,
    pub quantity: u32,
    pub involves_mock_order: bool,
}

#[derive(Debug, Clone)]
pub struct MatchOutcome {
    pub trades: Vec<Trade>,
    pub start_time: u64,
    pub end_time: u64,
}

impl MatchOutcome {
    pub fn new(cap: usize) -> Self {
        Self {
            trades: Vec::with_capacity(cap),
            start_time: 0,
            end_time: 0,
        }
    }

    pub fn add_trade(&mut self, trade: Trade) {
        self.trades.push(trade);
    }

    pub fn total_count(&self) -> u32 {
        self.trades.len() as u32
    }

    pub fn total_time(&self) -> u64 {
        self.end_time.saturating_sub(self.start_time)
    }

    pub fn time_per_trade(&self) -> u32 {
        if self.total_count() == 0 {
            return 0;
        }
        (self.total_time() / self.total_count() as u64) as u32
    }
}

#[derive(Debug)]
pub enum IncomingMessage {
    Order(OrderRequest),
    Cancel(CancelOrder),
}

pub type OrderIndex = u32;

pub trait ResultSender: Send + Sync {
    fn send_result(&self, result: MatchOutcome);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderBookError {
    PriceOutOfRange {
        price: u64,
        min_price: u64,
        max_price: u64,
    },
    PriceNotOnTick {
        price: u64,
        base_price: u64,
        tick: u64,
    },
}

impl fmt::Display for OrderBookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PriceOutOfRange {
                price,
                min_price,
                max_price,
            } => write!(
                f,
                "order price {} is out of dense order book range [{}, {}]",
                price, min_price, max_price
            ),
            Self::PriceNotOnTick {
                price,
                base_price,
                tick,
            } => write!(
                f,
                "order price {} is not aligned to tick {} from base price {}",
                price, tick, base_price
            ),
        }
    }
}

impl Error for OrderBookError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallAuctionPoolError {
    UnsupportedPriceType { price_type: PriceType },
}

impl fmt::Display for CallAuctionPoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPriceType { price_type } => {
                write!(f, "call auction only supports limit orders, got {:?}", price_type)
            }
        }
    }
}

impl Error for CallAuctionPoolError {}

#[derive(Debug, Clone)]
pub struct OrderSubmitError {
    pub order: OrderRequest,
    pub source: OrderBookError,
}

impl fmt::Display for OrderSubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "order_id={} product_id={} rejected: {}",
            self.order.order_id, self.order.product_id, self.source
        )
    }
}

impl Error for OrderSubmitError {}

#[derive(Debug, Clone)]
pub struct CallAuctionOrderSubmitError {
    pub order: OrderRequest,
    pub source: CallAuctionPoolError,
}

impl fmt::Display for CallAuctionOrderSubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "order_id={} product_id={} rejected by call auction: {}",
            self.order.order_id, self.order.product_id, self.source
        )
    }
}

impl Error for CallAuctionOrderSubmitError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuctionKind {
    Opening,
    Closing,
    VolatilityInterruption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketPhase {
    PreOpen,
    AuctionOrderEntry(AuctionKind),
    AuctionFrozen(AuctionKind),
    AuctionMatching(AuctionKind),
    ContinuousTrading,
    TradingHalt,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketStructureConfig {
    pub has_opening_auction: bool,
    pub has_closing_auction: bool,
    pub allows_volatility_auction: bool,
}

impl Default for MarketStructureConfig {
    fn default() -> Self {
        Self {
            has_opening_auction: true,
            has_closing_auction: false,
            allows_volatility_auction: false,
        }
    }
}

#[derive(Debug)]
pub enum SubmitOrderError {
    Continuous(OrderSubmitError),
    CallAuction(CallAuctionOrderSubmitError),
    InvalidPhase { phase: MarketPhase },
}

impl fmt::Display for SubmitOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Continuous(err) => write!(f, "{}", err),
            Self::CallAuction(err) => write!(f, "{}", err),
            Self::InvalidPhase { phase } => {
                write!(f, "order submission is not allowed during phase {:?}", phase)
            }
        }
    }
}

impl Error for SubmitOrderError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseTransitionError {
    InvalidTransition {
        from: MarketPhase,
        to: MarketPhase,
    },
    UnsupportedAuctionKind {
        kind: AuctionKind,
    },
    MissingActiveAuction {
        phase: MarketPhase,
    },
    ActiveAuctionAlreadyExists {
        kind: AuctionKind,
    },
    InvalidPhaseForAuctionExecution {
        phase: MarketPhase,
    },
}

impl fmt::Display for PhaseTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid market phase transition from {:?} to {:?}", from, to)
            }
            Self::UnsupportedAuctionKind { kind } => {
                write!(f, "auction kind {:?} is not enabled by market structure", kind)
            }
            Self::MissingActiveAuction { phase } => {
                write!(f, "phase {:?} requires an active auction session", phase)
            }
            Self::ActiveAuctionAlreadyExists { kind } => {
                write!(f, "cannot start {:?} auction because another auction session is active", kind)
            }
            Self::InvalidPhaseForAuctionExecution { phase } => {
                write!(f, "cannot execute call auction during phase {:?}", phase)
            }
        }
    }
}

impl Error for PhaseTransitionError {}

#[derive(Debug)]
pub enum TradingSessionError {
    Submit(SubmitOrderError),
    Transition(PhaseTransitionError),
}

impl fmt::Display for TradingSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Submit(err) => write!(f, "{}", err),
            Self::Transition(err) => write!(f, "{}", err),
        }
    }
}

impl Error for TradingSessionError {}

#[derive(Debug, Clone, Copy)]
pub struct MatchedRestingOrder {
    pub order_index: OrderIndex,
    pub matched_quantity: u32,
    pub is_buy: bool,
}

#[derive(Default, Clone, Debug)]
pub struct OrdersBucket {
    pub orders: VecDeque<RestingOrder>,
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
    pub order_map: AHashMap<u64, (bool, usize)>,
    pub total_bid_volume: u32,
    pub total_ask_volume: u32,
    pub last_outcome: MatchOutcome,
    pub timer: HighResolutionTimer,
}

#[derive(Debug)]
pub struct SparseOrderBook {
    pub bids: BTreeMap<u64, OrdersBucket>,
    pub asks: BTreeMap<u64, OrdersBucket>,
    pub order_map: AHashMap<u64, (bool, u64)>,
    pub total_bid_volume: u32,
    pub total_ask_volume: u32,
    pub last_outcome: MatchOutcome,
    pub tick: u64,
    pub base_price: u64,
    pub timer: HighResolutionTimer,
}

pub trait OrderBook: Send {
    fn seed_order(&mut self, order: OrderRequest) -> Result<(), OrderBookError>;
    fn match_order(&mut self, incoming: OrderRequest) -> Result<(), OrderBookError>;
    fn cancel_order(&mut self, order_id: u64) -> bool;
    fn last_outcome(&self) -> &MatchOutcome;
}

#[derive(Debug)]
pub enum AnyOrderBook {
    Dense(DenseOrderBook),
    Sparse(SparseOrderBook),
}

impl AnyOrderBook {
    pub fn dense(tick: u64, base_price: u64, max_levels: usize, trade_cap: usize) -> Self {
        Self::Dense(DenseOrderBook::new(tick, base_price, max_levels, trade_cap))
    }

    pub fn sparse(tick: u64, base_price: u64, max_levels: usize, trade_cap: usize) -> Self {
        Self::Sparse(SparseOrderBook::new(tick, base_price, max_levels, trade_cap))
    }
}

impl OrderBook for AnyOrderBook {
    fn seed_order(&mut self, order: OrderRequest) -> Result<(), OrderBookError> {
        match self {
            Self::Dense(book) => book.seed_order(order),
            Self::Sparse(book) => book.seed_order(order),
        }
    }

    fn match_order(&mut self, incoming: OrderRequest) -> Result<(), OrderBookError> {
        match self {
            Self::Dense(book) => book.match_order(incoming),
            Self::Sparse(book) => book.match_order(incoming),
        }
    }

    fn cancel_order(&mut self, order_id: u64) -> bool {
        match self {
            Self::Dense(book) => book.cancel_order(order_id),
            Self::Sparse(book) => book.cancel_order(order_id),
        }
    }

    fn last_outcome(&self) -> &MatchOutcome {
        match self {
            Self::Dense(book) => &book.last_outcome,
            Self::Sparse(book) => &book.last_outcome,
        }
    }
}

#[derive(Debug)]
pub struct EngineState {
    pub instance_tag: [u8; 16],
    pub product_id: u16,
    pub order_book: AnyOrderBook,
    pub market_structure: MarketStructureConfig,
    pub phase: MarketPhase,
    pub active_auction: Option<AuctionSession>,
    pub matched_orders: u64,
    pub total_received_orders: u64,
    pub start_time: u64,
}

#[derive(Debug)]
pub struct CallAuctionPool {
    pub bids: Vec<RestingOrder>,
    pub asks: Vec<RestingOrder>,
}

#[derive(Debug)]
pub struct AuctionSession {
    pub kind: AuctionKind,
    pub pool: CallAuctionPool,
    pub started_at: u64,
    pub frozen_at: Option<u64>,
    pub matched_at: Option<u64>,
    pub last_outcome: MatchOutcome,
}

impl OrderRequest {
    #[inline(always)]
    pub fn is_buy(&self) -> bool {
        self.side == OrderSide::Buy
    }

    #[inline(always)]
    pub fn is_sell(&self) -> bool {
        self.side == OrderSide::Sell
    }

    #[inline(always)]
    pub fn is_mocked_order(&self) -> bool {
        self.flags.is_mock
    }

    #[inline(always)]
    pub fn is_limit(&self) -> bool {
        self.price_type == PriceType::Limit
    }

    pub fn into_resting_order(self) -> RestingOrder {
        RestingOrder {
            product_id: self.product_id,
            order_id: self.order_id,
            side: self.side,
            price_type: self.price_type,
            flags: self.flags,
            price: self.price,
            original_quantity: self.quantity,
            remaining_quantity: self.quantity,
            submit_time: self.submit_time,
            expire_time: self.expire_time,
        }
    }
}

impl RestingOrder {
    #[inline(always)]
    pub fn is_buy(&self) -> bool {
        self.side == OrderSide::Buy
    }

    #[inline(always)]
    pub fn is_sell(&self) -> bool {
        self.side == OrderSide::Sell
    }

    #[inline(always)]
    pub fn is_mocked_order(&self) -> bool {
        self.flags.is_mock
    }

    #[inline(always)]
    pub fn is_limit(&self) -> bool {
        self.price_type == PriceType::Limit
    }
}
