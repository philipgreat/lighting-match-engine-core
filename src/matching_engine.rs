use std::time::{SystemTime, UNIX_EPOCH};

use crate::data_types::{
    EngineState, MatchResult, Order, ORDER_PRICE_TYPE_LIMIT, ORDER_TYPE_BUY, ORDER_TYPE_SELL,
};

#[derive(Debug, Clone)]
pub struct SubmitOutcome {
    pub resting_qty: u32,
    pub match_result: MatchResult,
}

#[derive(Debug, Clone)]
pub struct BookSnapshot {
    pub product_id: u16,
    pub best_bid: Option<u64>,
    pub best_ask: Option<u64>,
    pub bid_levels: usize,
    pub ask_levels: usize,
    pub total_bid_volume: u32,
    pub total_ask_volume: u32,
}

#[derive(Debug, Clone)]
pub struct EngineStats {
    pub product_id: u16,
    pub total_received_orders: u64,
    pub matched_orders: u64,
    pub start_time: u64,
}

pub fn tag_to_u16_array(tag: &str) -> [u8; 16] {
    let mut tag_array = [0u8; 16];
    let bytes = tag.as_bytes();
    let len = std::cmp::min(bytes.len(), 16);
    tag_array[..len].copy_from_slice(&bytes[..len]);
    tag_array
}

pub fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time drifted backwards")
        .as_nanos() as u64
}

impl EngineState {
    pub fn has_order(&self, order_id: u64) -> bool {
        self.order_book.order_map.contains_key(&order_id)
    }

    pub fn remaining_quantity(&self, order_id: u64) -> Option<u32> {
        let (is_buy, price) = *self.order_book.order_map.get(&order_id)?;
        let bucket = if is_buy {
            self.order_book.bids.get(&price)?
        } else {
            self.order_book.asks.get(&price)?
        };

        bucket
            .orders
            .iter()
            .find(|order| order.order_id == order_id)
            .map(|order| order.quantity)
    }

    pub fn submit_order(&mut self, order: Order) -> Result<SubmitOutcome, String> {
        if self.order_book.order_map.contains_key(&order.order_id) {
            return Err(format!("duplicate order id {}", order.order_id));
        }

        self.total_received_orders = self.total_received_orders.saturating_add(1);
        let original_qty = order.quantity;
        self.match_order(order);

        let result = self.order_book.match_result.clone();
        let executed_qty = result
            .order_execution_list
            .iter()
            .map(|execution| execution.quantity)
            .sum::<u32>();
        self.matched_orders = self
            .matched_orders
            .saturating_add(result.order_execution_list.len() as u64);

        Ok(SubmitOutcome {
            resting_qty: original_qty.saturating_sub(executed_qty),
            match_result: result,
        })
    }

    pub fn cancel_existing_order(&mut self, order_id: u64) -> bool {
        self.order_book.cancel_order(order_id)
    }

    pub fn book_snapshot(&self) -> BookSnapshot {
        BookSnapshot {
            product_id: self.product_id,
            best_bid: best_bid_price(self),
            best_ask: best_ask_price(self),
            bid_levels: self
                .order_book
                .bids
                .iter()
                .filter(|(_, bucket)| !bucket.orders.is_empty())
                .count(),
            ask_levels: self
                .order_book
                .asks
                .iter()
                .filter(|(_, bucket)| !bucket.orders.is_empty())
                .count(),
            total_bid_volume: self.order_book.total_bid_volume,
            total_ask_volume: self.order_book.total_ask_volume,
        }
    }

    pub fn stats_snapshot(&self) -> EngineStats {
        EngineStats {
            product_id: self.product_id,
            total_received_orders: self.total_received_orders,
            matched_orders: self.matched_orders,
            start_time: self.start_time,
        }
    }
}

pub fn best_bid_price(engine: &EngineState) -> Option<u64> {
    engine.order_book.bids.keys().next_back().copied()
}

pub fn best_ask_price(engine: &EngineState) -> Option<u64> {
    engine.order_book.asks.keys().next().copied()
}

pub fn make_benchmark_order(product_id: u16, order_id: u64, is_buy: bool, quantity: u32) -> Order {
    Order {
        product_id,
        order_side: if is_buy { ORDER_TYPE_BUY } else { ORDER_TYPE_SELL },
        price: if is_buy { 100000000000 } else { 1 },
        price_type: ORDER_PRICE_TYPE_LIMIT,
        quantity,
        order_id,
        submit_time: now_nanos(),
        expire_time: 0,
        _padding: [0u8; 24],
    }
}

#[cfg(test)]
mod tests {
    use crate::data_types::{ORDER_PRICE_TYPE_LIMIT, ORDER_TYPE_BUY, ORDER_TYPE_SELL};
    use crate::fix::model::NewOrderRequest;

    use super::{tag_to_u16_array, EngineState};

    fn test_engine(product_id: u16) -> EngineState {
        EngineState::new_for_redis_module(tag_to_u16_array("TEST"), product_id)
    }

    fn limit_order(
        order_id: u64,
        side: u8,
        quantity: u32,
        price: u64,
    ) -> crate::data_types::Order {
        NewOrderRequest {
            order_id,
            side,
            quantity,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            price,
            symbol: Some("AAPL".to_string()),
            transact_time: Some(123),
        }
        .into_order(7, 123)
    }

    #[test]
    fn submit_order_updates_book_snapshot() {
        let mut engine = test_engine(7);
        let outcome = engine
            .submit_order(limit_order(1001, ORDER_TYPE_BUY, 5, 101))
            .unwrap();

        assert_eq!(outcome.resting_qty, 5);
        assert!(outcome.match_result.order_execution_list.is_empty());

        let book = engine.book_snapshot();
        assert_eq!(book.best_bid, Some(101));
        assert_eq!(book.best_ask, None);
        assert_eq!(book.total_bid_volume, 5);
        assert_eq!(book.bid_levels, 1);
        assert!(engine.has_order(1001));
    }

    #[test]
    fn duplicate_order_id_is_rejected() {
        let mut engine = test_engine(7);
        engine
            .submit_order(limit_order(1001, ORDER_TYPE_BUY, 5, 101))
            .unwrap();

        let err = engine
            .submit_order(limit_order(1001, ORDER_TYPE_BUY, 3, 102))
            .unwrap_err();

        assert!(err.contains("duplicate order id 1001"));
    }

    #[test]
    fn crossing_orders_generate_trade_and_remove_resting_order() {
        let mut engine = test_engine(7);
        engine
            .submit_order(limit_order(1001, ORDER_TYPE_SELL, 4, 101))
            .unwrap();

        let outcome = engine
            .submit_order(limit_order(1002, ORDER_TYPE_BUY, 4, 101))
            .unwrap();

        assert_eq!(outcome.resting_qty, 0);
        assert_eq!(outcome.match_result.order_execution_list.len(), 1);
        let trade = &outcome.match_result.order_execution_list[0];
        assert_eq!(trade.buy_order_id, 1002);
        assert_eq!(trade.sell_order_id, 1001);
        assert_eq!(trade.quantity, 4);

        let book = engine.book_snapshot();
        assert_eq!(book.best_bid, None);
        assert_eq!(book.best_ask, None);
        assert_eq!(book.total_bid_volume, 0);
        assert_eq!(book.total_ask_volume, 0);
        assert!(!engine.has_order(1001));
    }

    #[test]
    fn cancel_existing_order_removes_it_from_book() {
        let mut engine = test_engine(7);
        engine
            .submit_order(limit_order(1001, ORDER_TYPE_BUY, 5, 101))
            .unwrap();

        assert!(engine.cancel_existing_order(1001));
        assert!(!engine.has_order(1001));

        let book = engine.book_snapshot();
        assert_eq!(book.best_bid, None);
        assert_eq!(book.total_bid_volume, 0);
    }

    #[test]
    fn manual_replace_flow_swaps_resting_order() {
        let mut engine = test_engine(7);
        engine
            .submit_order(limit_order(1001, ORDER_TYPE_BUY, 5, 101))
            .unwrap();

        assert!(engine.cancel_existing_order(1001));
        engine
            .submit_order(limit_order(1002, ORDER_TYPE_BUY, 8, 102))
            .unwrap();

        let book = engine.book_snapshot();
        assert_eq!(book.best_bid, Some(102));
        assert_eq!(book.total_bid_volume, 8);
        assert!(!engine.has_order(1001));
        assert!(engine.has_order(1002));
    }
}
