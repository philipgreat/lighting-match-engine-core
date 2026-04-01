use alloc::vec::Vec;
use core::cmp::min;

use crate::engine_core::types::{
    CallAuctionPool, CancelOrder, MatchResult, Order, OrderExecution, ORDER_TYPE_BUY,
    ORDER_TYPE_MOCK_BUY, ORDER_TYPE_MOCK_SELL, ORDER_TYPE_SELL,
};

impl CallAuctionPool {
    pub fn new(init_size: usize) -> Self {
        Self {
            bids: Vec::with_capacity(init_size),
            asks: Vec::with_capacity(init_size),
        }
    }

    pub fn add_order(&mut self, order: Order) {
        match order.order_side {
            ORDER_TYPE_BUY | ORDER_TYPE_MOCK_BUY => self.bids.push(order),
            ORDER_TYPE_SELL | ORDER_TYPE_MOCK_SELL => self.asks.push(order),
            _ => {}
        }
    }

    pub fn calculate_match_price_final(&self, price_tick: u64) -> Option<(u64, u32)> {
        if self.bids.is_empty() || self.asks.is_empty() || price_tick == 0 {
            return None;
        }

        let mut raw_prices: Vec<u64> = self
            .bids
            .iter()
            .map(|order| order.price)
            .chain(self.asks.iter().map(|order| order.price))
            .collect();
        raw_prices.sort_unstable();
        raw_prices.dedup();

        let mut critical_ticks = Vec::new();
        for price in raw_prices {
            let base = (price / price_tick) * price_tick;
            critical_ticks.push(base);
            critical_ticks.push(base + price_tick);
            if base >= price_tick {
                critical_ticks.push(base - price_tick);
            }
        }
        critical_ticks.sort_unstable();
        critical_ticks.dedup();

        let mut sorted_bids = self.bids.clone();
        sorted_bids.sort_by(|left, right| right.price.cmp(&left.price));

        let mut sorted_asks = self.asks.clone();
        sorted_asks.sort_by(|left, right| left.price.cmp(&right.price));

        let mut best_price = 0u64;
        let mut max_volume = 0u32;
        let mut min_imbalance = u32::MAX;

        let mut total_bid_vol: u32 = sorted_bids.iter().map(|order| order.quantity).sum();
        let mut total_ask_vol: u32 = 0;
        let mut ask_idx = 0;
        let mut bid_ptr = sorted_bids.len();

        for &test_price in &critical_ticks {
            while bid_ptr > 0 && sorted_bids[bid_ptr - 1].price < test_price {
                total_bid_vol -= sorted_bids[bid_ptr - 1].quantity;
                bid_ptr -= 1;
            }

            while ask_idx < sorted_asks.len() && sorted_asks[ask_idx].price <= test_price {
                total_ask_vol += sorted_asks[ask_idx].quantity;
                ask_idx += 1;
            }

            let current_vol = min(total_bid_vol, total_ask_vol);
            let imbalance = total_bid_vol.abs_diff(total_ask_vol);

            if current_vol > max_volume {
                max_volume = current_vol;
                best_price = test_price;
                min_imbalance = imbalance;
            } else if current_vol == max_volume && max_volume > 0 && imbalance < min_imbalance {
                best_price = test_price;
                min_imbalance = imbalance;
            }
        }

        if max_volume > 0 {
            Some((best_price, max_volume))
        } else {
            None
        }
    }

    pub fn execute_auction(
        &mut self,
        price_tick: u64,
        instance_tag: [u8; 16],
        product_id: u16,
        current_ts: u64,
    ) -> MatchResult {
        let mut match_result = MatchResult {
            order_execution_list: Vec::new(),
            start_time: current_ts,
            end_time: current_ts,
        };

        let (match_price, mut total_volume_to_match) = match self.calculate_match_price_final(price_tick)
        {
            Some(result) => result,
            None => return match_result,
        };

        let mut eligible_bids: Vec<Order> = self
            .bids
            .drain(..)
            .filter(|order| order.price >= match_price)
            .collect();
        eligible_bids
            .sort_by(|left, right| right.price.cmp(&left.price).then(left.submit_time.cmp(&right.submit_time)));

        let mut eligible_asks: Vec<Order> = self
            .asks
            .drain(..)
            .filter(|order| order.price <= match_price)
            .collect();
        eligible_asks
            .sort_by(|left, right| left.price.cmp(&right.price).then(left.submit_time.cmp(&right.submit_time)));

        let mut bid_idx = 0;
        let mut ask_idx = 0;

        while bid_idx < eligible_bids.len()
            && ask_idx < eligible_asks.len()
            && total_volume_to_match > 0
        {
            let bid = &mut eligible_bids[bid_idx];
            let ask = &mut eligible_asks[ask_idx];
            let match_qty = min(bid.quantity, min(ask.quantity, total_volume_to_match));

            if match_qty > 0 {
                match_result.add_order_execution(OrderExecution {
                    instance_tag,
                    product_id,
                    buy_order_id: bid.order_id,
                    sell_order_id: ask.order_id,
                    price: match_price,
                    quantity: match_qty,
                    trade_time_network: 0,
                    internal_match_time: 0,
                    is_mocked_result: bid.is_mocked_order() || ask.is_mocked_order(),
                });

                bid.quantity -= match_qty;
                ask.quantity -= match_qty;
                total_volume_to_match -= match_qty;
            }

            if bid.quantity == 0 {
                bid_idx += 1;
            }
            if ask.quantity == 0 {
                ask_idx += 1;
            }
        }

        self.bids
            .extend(eligible_bids.into_iter().filter(|order| order.quantity > 0));
        self.asks
            .extend(eligible_asks.into_iter().filter(|order| order.quantity > 0));

        match_result.end_time = 0;
        match_result
    }

    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }

    pub fn cancel_order(&mut self, cancel: &CancelOrder) -> bool {
        if let Some(pos) = self.bids.iter().position(|order| order.order_id == cancel.order_id) {
            self.bids.remove(pos);
            return true;
        }

        if let Some(pos) = self.asks.iter().position(|order| order.order_id == cancel.order_id) {
            self.asks.remove(pos);
            return true;
        }

        false
    }
}
