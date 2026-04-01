use alloc::collections::BTreeMap;
use alloc::vec;

use crate::engine_core::types::{
    DenseOrderBook, MatchResult, Order, OrderExecution, OrdersBucket, ORDER_PRICE_TYPE_LIMIT,
};

impl DenseOrderBook {
    pub fn new(tick: u64, base_price: u64, max_levels: usize, trade_cap: usize) -> Self {
        Self {
            bids: vec![OrdersBucket::default(); max_levels],
            asks: vec![OrdersBucket::default(); max_levels],
            best_bid: -1,
            best_ask: max_levels as isize,
            tick,
            base_price,
            levels: max_levels,
            order_map: BTreeMap::new(),
            total_bid_volume: 0,
            total_ask_volume: 0,
            match_result: MatchResult::new(trade_cap),
        }
    }

    #[inline(always)]
    fn price_to_index(&self, price: u64) -> usize {
        ((price - self.base_price) / self.tick) as usize
    }

    fn add_order(&mut self, order: Order) {
        let idx = self.price_to_index(order.price);
        if idx >= self.levels {
            return;
        }

        if order.is_buy() {
            self.bids[idx].orders.push_back(order.clone());
            self.best_bid = self.best_bid.max(idx as isize);
            self.total_bid_volume += order.quantity;
            self.order_map.insert(order.order_id, (true, idx));
        } else {
            self.asks[idx].orders.push_back(order.clone());
            self.best_ask = self.best_ask.min(idx as isize);
            self.total_ask_volume += order.quantity;
            self.order_map.insert(order.order_id, (false, idx));
        }
    }

    pub fn fuel_order(&mut self, order: Order) {
        self.add_order(order);
    }

    pub fn match_order(&mut self, mut order: Order) {
        self.match_result.order_execution_list.clear();

        if order.is_buy() {
            self.match_buy(&mut order);
        } else {
            self.match_sell(&mut order);
        }

        if order.quantity > 0 && order.price_type == ORDER_PRICE_TYPE_LIMIT {
            self.add_order(order);
        }
    }

    fn match_buy(&mut self, order: &mut Order) {
        while order.quantity > 0 && self.best_ask <= self.best_bid {
            let idx = self.best_ask as usize;
            let bucket = &mut self.asks[idx];

            if bucket.orders.is_empty() {
                self.best_ask += 1;
                continue;
            }

            let resting = bucket.orders.front_mut().unwrap();
            if order.price_type == ORDER_PRICE_TYPE_LIMIT && order.price < resting.price {
                break;
            }

            let qty = order.quantity.min(resting.quantity);
            order.quantity -= qty;
            resting.quantity -= qty;
            self.total_ask_volume -= qty;

            self.match_result.add_order_execution(OrderExecution {
                instance_tag: [0; 16],
                product_id: order.product_id,
                buy_order_id: order.order_id,
                sell_order_id: resting.order_id,
                price: resting.price,
                quantity: qty,
                trade_time_network: 0,
                internal_match_time: 0,
                is_mocked_result: order.is_mocked_order(),
            });

            if resting.quantity == 0 {
                let filled = bucket.orders.pop_front().unwrap();
                self.order_map.remove(&filled.order_id);
            }
        }
    }

    fn match_sell(&mut self, order: &mut Order) {
        while order.quantity > 0 && self.best_bid >= self.best_ask {
            let idx = self.best_bid as usize;
            let bucket = &mut self.bids[idx];

            if bucket.orders.is_empty() {
                self.best_bid -= 1;
                continue;
            }

            let resting = bucket.orders.front_mut().unwrap();
            if order.price_type == ORDER_PRICE_TYPE_LIMIT && order.price > resting.price {
                break;
            }

            let qty = order.quantity.min(resting.quantity);
            order.quantity -= qty;
            resting.quantity -= qty;
            self.total_bid_volume -= qty;

            self.match_result.add_order_execution(OrderExecution {
                instance_tag: [0; 16],
                product_id: order.product_id,
                buy_order_id: resting.order_id,
                sell_order_id: order.order_id,
                price: resting.price,
                quantity: qty,
                trade_time_network: 0,
                internal_match_time: 0,
                is_mocked_result: order.is_mocked_order(),
            });

            if resting.quantity == 0 {
                let filled = bucket.orders.pop_front().unwrap();
                self.order_map.remove(&filled.order_id);
            }
        }
    }

    pub fn cancel_order(&mut self, order_id: u64) -> bool {
        let (is_buy, idx) = match self.order_map.remove(&order_id) {
            Some(value) => value,
            None => return false,
        };

        let bucket = if is_buy {
            &mut self.bids[idx]
        } else {
            &mut self.asks[idx]
        };

        if let Some(pos) = bucket.orders.iter().position(|order| order.order_id == order_id) {
            let removed = bucket.orders.remove(pos).unwrap();
            if is_buy {
                self.total_bid_volume -= removed.quantity;
            } else {
                self.total_ask_volume -= removed.quantity;
            }
            return true;
        }

        false
    }
}
