use crate::types::{AnyOrderBook, CallAuctionPool, EngineState, OrderBook, OrderFlags, OrderRequest, OrderSide, OrderSubmitError, PriceType};

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

impl EngineState {
    pub fn new(instance_tag: [u8; 16], product_id: u16, order_book: AnyOrderBook) -> Self {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        EngineState {
            instance_tag,
            product_id,
            order_book,
            call_auction_pool: CallAuctionPool::new(1000),
            matched_orders: 0,
            total_received_orders: 0,
            start_time: now_nanos,
        }
    }

    pub fn increase_match(&mut self) {
        self.matched_orders += 1;
    }

    pub fn match_order(&mut self, new_order: OrderRequest) -> Result<(), OrderSubmitError> {
        self.total_received_orders += 1;
        self.order_book.match_order(new_order.clone()).map_err(|source| OrderSubmitError {
            order: new_order,
            source,
        })?;
        self.matched_orders += self.order_book.last_outcome().total_count() as u64;
        Ok(())
    }

    pub fn load_sample_test_book(&mut self, test_order_book_size: u32) -> Result<(), OrderSubmitError> {
        for i in 0..test_order_book_size {
            let order = self.create_buy_order(i);
            self.order_book.seed_order(order.clone()).map_err(|source| OrderSubmitError {
                order,
                source,
            })?;
        }
        for i in 0..test_order_book_size {
            let order = self.create_sell_order(i, test_order_book_size);
            self.order_book.seed_order(order.clone()).map_err(|source| OrderSubmitError {
                order,
                source,
            })?;
        }
        Ok(())
    }

    pub fn create_buy_order(&self, index: u32) -> OrderRequest {
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;
        OrderRequest {
            product_id: self.product_id,
            order_id: (index + 1) as u64,
            side: OrderSide::Buy,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: (index + 1) as u64,
            quantity: 2,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
            _padding: [0u8; 24],
        }
    }

    pub fn create_sell_order(&self, index: u32, size: u32) -> OrderRequest {
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        OrderRequest {
            product_id: self.product_id,
            order_id: (size + index + 1) as u64,
            side: OrderSide::Sell,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: (size + 1 + index) as u64,
            quantity: 2,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
            _padding: [0u8; 24],
        }
    }
}

pub struct StatusBroadcaster {
    state: Arc<EngineState>,
}

impl StatusBroadcaster {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AnyOrderBook, OrderBookError};

    #[test]
    fn engine_state_wraps_order_book_error_with_order_context() {
        let order_book = AnyOrderBook::dense(10, 100, 4, 8);
        let mut state = EngineState::new([0; 16], 7, order_book);

        let order = OrderRequest {
            product_id: 7,
            order_id: 99,
            side: OrderSide::Buy,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            price: 105,
            quantity: 1,
            submit_time: 0,
            expire_time: 0,
            _padding: [0u8; 24],
        };

        let err = state.match_order(order.clone()).unwrap_err();
        assert_eq!(err.order.order_id, 99);
        assert_eq!(err.order.product_id, 7);
        assert_eq!(
            err.source,
            OrderBookError::PriceNotOnTick {
                price: 105,
                base_price: 100,
                tick: 10,
            }
        );
    }
}
