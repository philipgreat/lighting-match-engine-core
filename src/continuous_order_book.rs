// ================================
// continuous_order_book.rs
// ================================

use ahash::AHashMap;
use std::collections::VecDeque;

use crate::data_types::*;
use crate::date_time_tool::current_timestamp;
use crate::high_resolution_timer::HighResolutionTimer;

// --- FIFO bucket per price ---


// --- Price Ladder Order Book ---


impl ContinuousOrderBook {
    // ----------------------------
    // Init
    // ----------------------------
    pub fn new(
        tick: u64,
        base_price: u64,
        max_levels: usize,
        trade_cap: usize,
    ) -> Self {
        Self {
            bids: vec![OrdersBucket::default(); max_levels],
            asks: vec![OrdersBucket::default(); max_levels],
            best_bid: -1,
            best_ask: max_levels as isize,
            tick,
            base_price,
            levels: max_levels,
            order_map: AHashMap::with_capacity(1024),
            total_bid_volumn: 0,
            total_ask_volumn: 0,
            match_result: MatchResult::new(trade_cap),
            timer:HighResolutionTimer::start(25*100_000_000), 
            //most cpu runs on this frequency, change to higher if you are using higher frequency CPU
        }
    }

    #[inline(always)]
    fn price_to_index(&self, price: u64) -> usize {
        //println!("{:?}", (price,self.base_price,self.tick));
        ((price - self.base_price) / self.tick) as usize
    }

    // ----------------------------
    // Add resting order
    // ----------------------------
    fn add_order(&mut self, order: Order) {
        let idx = self.price_to_index(order.price);

        if order.is_buy() {
            self.bids[idx].orders.push_back(order.clone());
            self.best_bid = self.best_bid.max(idx as isize);
            self.total_bid_volumn += order.quantity;
            self.order_map.insert(order.order_id, (true, idx));
        } else {
            self.asks[idx].orders.push_back(order.clone());
            self.best_ask = self.best_ask.min(idx as isize);
            self.total_ask_volumn += order.quantity;
            self.order_map.insert(order.order_id, (false, idx));
        }
    }
    pub fn fuel_order(&mut self, order: Order){
        self.add_order(order);
    }

    // ----------------------------
    // Public match entry
    // ----------------------------
    pub fn match_order(&mut self, mut order: Order) {
        self.match_result.order_execution_list.clear();
        self.match_result.start_time = self.timer.ns() as u64;

        if order.is_buy() {
            self.match_buy(&mut order);
        } else {
            self.match_sell(&mut order);
        }

        if order.quantity > 0 && order.price_type == ORDER_PRICE_TYPE_LIMIT {
            self.add_order(order);
        }
        
        self.match_result.end_time = self.timer.ns() as u64;
    }

    // ----------------------------
    // BUY vs ASK
    // ----------------------------
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
            self.total_ask_volumn -= qty;

            self.match_result.order_execution_list.push(OrderExecution {
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
                let o = bucket.orders.pop_front().unwrap();
                self.order_map.remove(&o.order_id);
            }
        }
    }

    // ----------------------------
    // SELL vs BID
    // ----------------------------
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
            self.total_bid_volumn -= qty;

            self.match_result.order_execution_list.push(OrderExecution {
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
                let o = bucket.orders.pop_front().unwrap();
                self.order_map.remove(&o.order_id);
            }
        }
    }

    // ----------------------------
    // Cancel order (O(1))
    // ----------------------------
    pub fn cancel_order(&mut self, order_id: u64) -> bool {
        let (is_buy, idx) = match self.order_map.remove(&order_id) {
            Some(v) => v,
            None => return false,
        };

        let bucket = if is_buy {
            &mut self.bids[idx]
        } else {
            &mut self.asks[idx]
        };

        if let Some(pos) = bucket.orders.iter().position(|o| o.order_id == order_id) {
            let o = bucket.orders.remove(pos).unwrap();
            if is_buy {
                self.total_bid_volumn -= o.quantity;
            } else {
                self.total_ask_volumn -= o.quantity;
            }
            return true;
        }
        false
    }
}
