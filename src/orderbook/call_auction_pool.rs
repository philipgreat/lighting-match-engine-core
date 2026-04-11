use crate::types::*;
use std::cmp::min;

impl CallAuctionPool {
    pub fn new(init_size: usize) -> Self {
        Self {
            bids: Vec::with_capacity(init_size),
            asks: Vec::with_capacity(init_size),
        }
    }

    pub fn add_order(&mut self, order: OrderRequest) {
        let resting = order.into_resting_order();
        if resting.is_buy() {
            self.bids.push(resting);
        } else if resting.is_sell() {
            self.asks.push(resting);
        }
    }

    pub fn calculate_match_price_final(&self, price_tick: u64) -> Option<(u64, u32)> {
        if self.bids.is_empty() || self.asks.is_empty() || price_tick == 0 {
            return None;
        }

        let mut raw_prices: Vec<u64> = self
            .bids
            .iter()
            .map(|o| o.price)
            .chain(self.asks.iter().map(|o| o.price))
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
        sorted_bids.sort_by(|a, b| b.price.cmp(&a.price));

        let mut sorted_asks = self.asks.clone();
        sorted_asks.sort_by(|a, b| a.price.cmp(&b.price));

        let mut best_price = 0u64;
        let mut max_volume = 0u32;
        let mut min_imbalance = u32::MAX;

        let mut total_bid_volume: u32 = sorted_bids.iter().map(|o| o.remaining_quantity).sum();
        let mut total_ask_volume: u32 = 0;
        let mut ask_idx = 0;
        let mut bid_ptr = sorted_bids.len();

        for &test_price in &critical_ticks {
            while bid_ptr > 0 && sorted_bids[bid_ptr - 1].price < test_price {
                total_bid_volume -= sorted_bids[bid_ptr - 1].remaining_quantity;
                bid_ptr -= 1;
            }
            while ask_idx < sorted_asks.len() && sorted_asks[ask_idx].price <= test_price {
                total_ask_volume += sorted_asks[ask_idx].remaining_quantity;
                ask_idx += 1;
            }

            let current_volume = min(total_bid_volume, total_ask_volume);
            let imbalance = total_bid_volume.abs_diff(total_ask_volume);

            if current_volume > max_volume {
                max_volume = current_volume;
                best_price = test_price;
                min_imbalance = imbalance;
            } else if current_volume == max_volume && max_volume > 0 && imbalance < min_imbalance {
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
        _instance_tag: [u8; 16],
        _product_id: u16,
        current_ts: u64,
    ) -> MatchOutcome {
        let mut outcome = MatchOutcome {
            trades: Vec::new(),
            start_time: current_ts,
            end_time: current_ts,
        };

        let (match_price, mut total_volume_to_match) = match self.calculate_match_price_final(price_tick) {
            Some(result) => result,
            None => return outcome,
        };

        let drained_bids: Vec<_> = self.bids.drain(..).collect();
        let (mut eligible_bids, remaining_bids): (Vec<_>, Vec<_>) = drained_bids
            .into_iter()
            .partition(|o| o.price >= match_price);
        eligible_bids.sort_by(|a, b| b.price.cmp(&a.price).then(a.submit_time.cmp(&b.submit_time)));

        let drained_asks: Vec<_> = self.asks.drain(..).collect();
        let (mut eligible_asks, remaining_asks): (Vec<_>, Vec<_>) = drained_asks
            .into_iter()
            .partition(|o| o.price <= match_price);
        eligible_asks.sort_by(|a, b| a.price.cmp(&b.price).then(a.submit_time.cmp(&b.submit_time)));

        let mut bid_idx = 0;
        let mut ask_idx = 0;

        while bid_idx < eligible_bids.len()
            && ask_idx < eligible_asks.len()
            && total_volume_to_match > 0
        {
            let bid = &mut eligible_bids[bid_idx];
            let ask = &mut eligible_asks[ask_idx];

            let match_qty = min(
                bid.remaining_quantity,
                min(ask.remaining_quantity, total_volume_to_match),
            );

            if match_qty > 0 {
                outcome.add_trade(Trade {
                    product_id: bid.product_id,
                    buy_order_id: bid.order_id,
                    sell_order_id: ask.order_id,
                    price: match_price,
                    quantity: match_qty,
                    involves_mock_order: bid.is_mocked_order() || ask.is_mocked_order(),
                });

                bid.remaining_quantity -= match_qty;
                ask.remaining_quantity -= match_qty;
                total_volume_to_match -= match_qty;
            }

            if bid.remaining_quantity == 0 {
                bid_idx += 1;
            }
            if ask.remaining_quantity == 0 {
                ask_idx += 1;
            }
        }

        self.bids.extend(remaining_bids);
        self.bids
            .extend(eligible_bids.into_iter().filter(|o| o.remaining_quantity > 0));
        self.asks.extend(remaining_asks);
        self.asks
            .extend(eligible_asks.into_iter().filter(|o| o.remaining_quantity > 0));

        outcome.end_time = current_ts;
        outcome
    }

    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }

    pub fn cancel_order(&mut self, cancel: &CancelOrder) -> bool {
        if let Some(pos) = self.bids.iter().position(|o| o.order_id == cancel.order_id) {
            self.bids.remove(pos);
            return true;
        }

        if let Some(pos) = self.asks.iter().position(|o| o.order_id == cancel.order_id) {
            self.asks.remove(pos);
            return true;
        }

        false
    }
}
