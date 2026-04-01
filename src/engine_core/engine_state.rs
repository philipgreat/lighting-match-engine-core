use crate::engine_core::types::{CallAuctionPool, EngineState, Order};

impl EngineState {
    pub fn new_with_start_time(instance_tag: [u8; 16], product_id: u16, start_time: u64) -> Self {
        Self {
            instance_tag,
            product_id,
            order_book: crate::engine_core::types::DenseOrderBook::new(100000, 1, 1_000_000, 100),
            call_auction_pool: CallAuctionPool::new(1000),
            matched_orders: 0,
            total_received_orders: 0,
            start_time,
        }
    }

    pub fn increase_match(&mut self) {
        self.matched_orders += 1;
    }

    pub fn match_order(&mut self, new_order: Order) {
        self.order_book.match_order(new_order);
    }
}
