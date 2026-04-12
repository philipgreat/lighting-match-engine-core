use crate::types::*;
use std::cmp::Ordering;
use std::cmp::min;

impl CallAuctionPool {
    fn add_volume(volume_map: &mut std::collections::BTreeMap<u64, u32>, price: u64, qty: u32) {
        *volume_map.entry(price).or_insert(0) += qty;
    }

    fn subtract_volume(
        volume_map: &mut std::collections::BTreeMap<u64, u32>,
        price: u64,
        qty: u32,
    ) {
        if let Some(level_qty) = volume_map.get_mut(&price) {
            *level_qty -= qty;
            if *level_qty == 0 {
                volume_map.remove(&price);
            }
        }
    }

    pub(crate) fn rebuild_price_levels(&mut self) {
        self.order_map.clear();
        self.bid_volume_by_price.clear();
        self.ask_volume_by_price.clear();

        for (idx, order) in self.bids.iter().enumerate() {
            if order.is_active() {
                self.order_map.insert(order.order_id, (true, idx));
                Self::add_volume(&mut self.bid_volume_by_price, order.price, order.remaining_quantity);
            }
        }

        for (idx, order) in self.asks.iter().enumerate() {
            if order.is_active() {
                self.order_map.insert(order.order_id, (false, idx));
                Self::add_volume(&mut self.ask_volume_by_price, order.price, order.remaining_quantity);
            }
        }
    }

    fn sort_eligible_levels(levels: &mut std::collections::BTreeMap<u64, Vec<RestingOrder>>) {
        for orders in levels.values_mut() {
            orders.sort_by_key(|order| order.submit_time);
        }
    }

    pub fn new(init_size: usize) -> Self {
        Self {
            bids: Vec::with_capacity(init_size),
            asks: Vec::with_capacity(init_size),
            order_map: ahash::AHashMap::with_capacity(init_size),
            bid_volume_by_price: std::collections::BTreeMap::new(),
            ask_volume_by_price: std::collections::BTreeMap::new(),
            raw_prices_buf: Vec::with_capacity(init_size * 2),
            critical_ticks_buf: Vec::with_capacity(init_size * 4),
            bid_levels_buf: Vec::with_capacity(init_size),
            ask_levels_buf: Vec::with_capacity(init_size),
            eligible_bid_orders_by_price: std::collections::BTreeMap::new(),
            eligible_ask_orders_by_price: std::collections::BTreeMap::new(),
            drained_bids_buf: Vec::with_capacity(init_size),
            drained_asks_buf: Vec::with_capacity(init_size),
            eligible_bids_buf: Vec::with_capacity(init_size),
            eligible_asks_buf: Vec::with_capacity(init_size),
            remaining_bids_buf: Vec::with_capacity(init_size),
            remaining_asks_buf: Vec::with_capacity(init_size),
        }
    }

    pub fn add_order(&mut self, order: OrderRequest) -> Result<(), CallAuctionOrderSubmitError> {
        if !order.is_limit() {
            let price_type = order.price_type;
            return Err(CallAuctionOrderSubmitError {
                order,
                source: CallAuctionPoolError::UnsupportedPriceType {
                    price_type,
                },
            });
        }

        let resting = order.into_resting_order();
        if resting.is_buy() {
            Self::add_volume(
                &mut self.bid_volume_by_price,
                resting.price,
                resting.remaining_quantity,
            );
            self.order_map.insert(resting.order_id, (true, self.bids.len()));
            self.bids.push(resting);
        } else if resting.is_sell() {
            Self::add_volume(
                &mut self.ask_volume_by_price,
                resting.price,
                resting.remaining_quantity,
            );
            self.order_map.insert(resting.order_id, (false, self.asks.len()));
            self.asks.push(resting);
        }

        Ok(())
    }

    pub fn calculate_match_price_final(&mut self, price_tick: u64) -> Option<(u64, u32)> {
        if self.bids.is_empty() || self.asks.is_empty() || price_tick == 0 {
            return None;
        }

        self.raw_prices_buf.clear();
        self.raw_prices_buf.extend(self.bid_volume_by_price.keys().copied());
        self.raw_prices_buf.extend(self.ask_volume_by_price.keys().copied());
        if self.raw_prices_buf.is_empty() {
            return None;
        }
        self.raw_prices_buf.sort_unstable();
        self.raw_prices_buf.dedup();

        self.critical_ticks_buf.clear();
        for &price in &self.raw_prices_buf {
            let base = (price / price_tick) * price_tick;
            self.critical_ticks_buf.push(base);
            self.critical_ticks_buf.push(base + price_tick);
            if base >= price_tick {
                self.critical_ticks_buf.push(base - price_tick);
            }
        }
        self.critical_ticks_buf.sort_unstable();
        self.critical_ticks_buf.dedup();

        self.bid_levels_buf.clear();
        self.bid_levels_buf
            .extend(self.bid_volume_by_price.iter().rev().map(|(&price, &qty)| (price, qty)));
        self.ask_levels_buf.clear();
        self.ask_levels_buf
            .extend(self.ask_volume_by_price.iter().map(|(&price, &qty)| (price, qty)));

        let mut best_price = 0u64;
        let mut max_volume = 0u32;
        let mut min_imbalance = u32::MAX;
        let mut has_candidate = false;

        let mut total_bid_volume: u32 = self
            .bid_levels_buf
            .iter()
            .map(|(_, qty)| *qty)
            .sum();
        let mut total_ask_volume: u32 = 0;
        let mut ask_idx = 0;
        let mut bid_ptr = self.bid_levels_buf.len();

        for &test_price in &self.critical_ticks_buf {
            while bid_ptr > 0 && self.bid_levels_buf[bid_ptr - 1].0 < test_price {
                total_bid_volume -= self.bid_levels_buf[bid_ptr - 1].1;
                bid_ptr -= 1;
            }
            while ask_idx < self.ask_levels_buf.len()
                && self.ask_levels_buf[ask_idx].0 <= test_price
            {
                total_ask_volume += self.ask_levels_buf[ask_idx].1;
                ask_idx += 1;
            }

            let current_volume = min(total_bid_volume, total_ask_volume);
            let imbalance = total_bid_volume.abs_diff(total_ask_volume);

            if current_volume == 0 {
                continue;
            }

            if !has_candidate
                || current_volume > max_volume
                || (current_volume == max_volume && imbalance < min_imbalance)
                || (current_volume == max_volume
                    && imbalance == min_imbalance
                    && Self::prefer_price_on_tie(
                        test_price,
                        best_price,
                        total_bid_volume,
                        total_ask_volume,
                    ))
            {
                has_candidate = true;
                max_volume = current_volume;
                best_price = test_price;
                min_imbalance = imbalance;
            }
        }

        if has_candidate {
            Some((best_price, max_volume))
        } else {
            None
        }
    }

    fn prefer_price_on_tie(
        candidate_price: u64,
        current_best_price: u64,
        total_bid_volume: u32,
        total_ask_volume: u32,
    ) -> bool {
        match total_bid_volume.cmp(&total_ask_volume) {
            Ordering::Greater => candidate_price > current_best_price,
            Ordering::Less => candidate_price < current_best_price,
            Ordering::Equal => candidate_price < current_best_price,
        }
    }

    pub fn execute_auction_into(
        &mut self,
        outcome: &mut MatchOutcome,
        price_tick: u64,
        _instance_tag: [u8; 16],
        _product_id: u16,
        current_ts: u64,
    ) {
        outcome.reset(current_ts);

        let (match_price, mut total_volume_to_match) = match self.calculate_match_price_final(price_tick) {
            Some(result) => result,
            None => return,
        };

        self.drained_bids_buf.clear();
        self.drained_bids_buf.extend(self.bids.drain(..));
        self.eligible_bid_orders_by_price.clear();
        self.remaining_bids_buf.clear();
        for order in self.drained_bids_buf.drain(..) {
            if !order.is_active() {
                continue;
            }
            if order.price >= match_price {
                self.eligible_bid_orders_by_price
                    .entry(order.price)
                    .or_default()
                    .push(order);
            } else {
                self.remaining_bids_buf.push(order);
            }
        }
        Self::sort_eligible_levels(&mut self.eligible_bid_orders_by_price);

        self.drained_asks_buf.clear();
        self.drained_asks_buf.extend(self.asks.drain(..));
        self.eligible_ask_orders_by_price.clear();
        self.remaining_asks_buf.clear();
        for order in self.drained_asks_buf.drain(..) {
            if !order.is_active() {
                continue;
            }
            if order.price <= match_price {
                self.eligible_ask_orders_by_price
                    .entry(order.price)
                    .or_default()
                    .push(order);
            } else {
                self.remaining_asks_buf.push(order);
            }
        }
        Self::sort_eligible_levels(&mut self.eligible_ask_orders_by_price);

        let bid_prices_desc: Vec<u64> = self
            .eligible_bid_orders_by_price
            .keys()
            .copied()
            .rev()
            .collect();
        let ask_prices_asc: Vec<u64> = self
            .eligible_ask_orders_by_price
            .keys()
            .copied()
            .collect();
        let mut bid_level_idx = 0usize;
        let mut ask_level_idx = 0usize;
        let mut bid_order_idx = 0usize;
        let mut ask_order_idx = 0usize;

        while bid_level_idx < bid_prices_desc.len()
            && ask_level_idx < ask_prices_asc.len()
            && total_volume_to_match > 0
        {
            let bid_price = bid_prices_desc[bid_level_idx];
            let ask_price = ask_prices_asc[ask_level_idx];

            let bid_orders = self
                .eligible_bid_orders_by_price
                .get_mut(&bid_price)
                .expect("bid level must exist");
            while bid_order_idx < bid_orders.len() && bid_orders[bid_order_idx].remaining_quantity == 0 {
                bid_order_idx += 1;
            }
            if bid_order_idx >= bid_orders.len() {
                bid_level_idx += 1;
                bid_order_idx = 0;
                continue;
            }

            let ask_orders = self
                .eligible_ask_orders_by_price
                .get_mut(&ask_price)
                .expect("ask level must exist");
            while ask_order_idx < ask_orders.len() && ask_orders[ask_order_idx].remaining_quantity == 0 {
                ask_order_idx += 1;
            }
            if ask_order_idx >= ask_orders.len() {
                ask_level_idx += 1;
                ask_order_idx = 0;
                continue;
            }

            let bid = &mut bid_orders[bid_order_idx];
            let ask = &mut ask_orders[ask_order_idx];
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
                bid_order_idx += 1;
            }
            if ask.remaining_quantity == 0 {
                ask_order_idx += 1;
            }
        }

        self.bids.extend(self.remaining_bids_buf.drain(..));
        for (_, orders) in self.eligible_bid_orders_by_price.iter_mut().rev() {
            for order in orders.drain(..) {
                if order.remaining_quantity > 0 {
                    self.bids.push(order);
                }
            }
        }
        self.asks.extend(self.remaining_asks_buf.drain(..));
        for orders in self.eligible_ask_orders_by_price.values_mut() {
            for order in orders.drain(..) {
                if order.remaining_quantity > 0 {
                    self.asks.push(order);
                }
            }
        }
        self.rebuild_price_levels();

        outcome.end_time = current_ts;
    }

    pub fn execute_auction(
        &mut self,
        price_tick: u64,
        instance_tag: [u8; 16],
        product_id: u16,
        current_ts: u64,
    ) -> MatchOutcome {
        let mut outcome = MatchOutcome::new(self.eligible_bids_buf.capacity().min(self.eligible_asks_buf.capacity()));
        self.execute_auction_into(&mut outcome, price_tick, instance_tag, product_id, current_ts);
        outcome
    }

    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
        self.order_map.clear();
        self.bid_volume_by_price.clear();
        self.ask_volume_by_price.clear();
    }

    pub fn cancel_order(&mut self, cancel: &CancelOrder) -> bool {
        let (is_buy, idx) = match self.order_map.remove(&cancel.order_id) {
            Some(v) => v,
            None => return false,
        };
        let orders = if is_buy {
            &mut self.bids
        } else {
            &mut self.asks
        };

        if idx < orders.len() && orders[idx].order_id == cancel.order_id && orders[idx].is_active() {
            let removed = orders.swap_remove(idx);
            debug_assert_eq!(removed.order_id, cancel.order_id);
            if is_buy {
                Self::subtract_volume(
                    &mut self.bid_volume_by_price,
                    removed.price,
                    removed.remaining_quantity,
                );
            } else {
                Self::subtract_volume(
                    &mut self.ask_volume_by_price,
                    removed.price,
                    removed.remaining_quantity,
                );
            }

            if idx < orders.len() {
                let moved = &orders[idx];
                self.order_map.insert(moved.order_id, (is_buy, idx));
            }

            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_order(
        order_id: u64,
        side: OrderSide,
        price: u64,
        quantity: u32,
        submit_time: u64,
    ) -> OrderRequest {
        make_order_with_price_type(order_id, side, PriceType::Limit, price, quantity, submit_time)
    }

    fn make_order_with_price_type(
        order_id: u64,
        side: OrderSide,
        price_type: PriceType,
        price: u64,
        quantity: u32,
        submit_time: u64,
    ) -> OrderRequest {
        OrderRequest {
            product_id: 7,
            side,
            price_type,
            flags: OrderFlags::default(),
            quantity,
            order_id,
            price,
            submit_time,
            expire_time: 0,
            _padding: [0u8; 24],
        }
    }

    #[test]
    fn calculate_match_price_returns_none_without_crossing_sides_or_tick() {
        let mut pool = CallAuctionPool::new(4);
        pool.add_order(make_order(1, OrderSide::Buy, 100, 2, 1)).unwrap();
        assert_eq!(pool.calculate_match_price_final(1), None);

        pool.add_order(make_order(2, OrderSide::Sell, 100, 2, 2)).unwrap();
        assert_eq!(pool.calculate_match_price_final(0), None);
    }

    #[test]
    fn calculate_match_price_finds_maximum_cross_volume() {
        let mut pool = CallAuctionPool::new(8);
        pool.add_order(make_order(1, OrderSide::Buy, 105, 3, 1)).unwrap();
        pool.add_order(make_order(2, OrderSide::Buy, 100, 2, 2)).unwrap();
        pool.add_order(make_order(3, OrderSide::Sell, 95, 1, 3)).unwrap();
        pool.add_order(make_order(4, OrderSide::Sell, 100, 4, 4)).unwrap();

        assert_eq!(pool.calculate_match_price_final(5), Some((100, 5)));
    }

    #[test]
    fn calculate_match_price_breaks_full_ties_toward_lower_price_when_balanced() {
        let mut pool = CallAuctionPool::new(8);
        pool.add_order(make_order(1, OrderSide::Buy, 110, 5, 1)).unwrap();
        pool.add_order(make_order(2, OrderSide::Buy, 100, 5, 2)).unwrap();
        pool.add_order(make_order(3, OrderSide::Sell, 100, 5, 3)).unwrap();
        pool.add_order(make_order(4, OrderSide::Sell, 110, 5, 4)).unwrap();

        assert_eq!(pool.calculate_match_price_final(10), Some((100, 5)));
    }

    #[test]
    fn calculate_match_price_breaks_ties_toward_higher_price_when_buy_surplus_remains() {
        let mut pool = CallAuctionPool::new(8);
        pool.add_order(make_order(1, OrderSide::Buy, 120, 5, 1)).unwrap();
        pool.add_order(make_order(2, OrderSide::Buy, 110, 10, 2)).unwrap();
        pool.add_order(make_order(3, OrderSide::Sell, 100, 10, 3)).unwrap();

        assert_eq!(pool.calculate_match_price_final(10), Some((110, 10)));
    }

    #[test]
    fn execute_auction_preserves_ineligible_orders_and_unfilled_remainders() {
        let mut pool = CallAuctionPool::new(8);
        pool.add_order(make_order(1, OrderSide::Buy, 110, 5, 1)).unwrap();
        pool.add_order(make_order(2, OrderSide::Buy, 90, 3, 2)).unwrap();
        pool.add_order(make_order(3, OrderSide::Sell, 100, 2, 3)).unwrap();
        pool.add_order(make_order(4, OrderSide::Sell, 100, 4, 4)).unwrap();
        pool.add_order(make_order(5, OrderSide::Sell, 120, 6, 5)).unwrap();

        let outcome = pool.execute_auction(10, [0; 16], 7, 123);

        assert_eq!(outcome.start_time, 123);
        assert_eq!(outcome.end_time, 123);
        assert_eq!(outcome.total_count(), 2);
        assert_eq!(outcome.trades[0].buy_order_id, 1);
        assert_eq!(outcome.trades[0].sell_order_id, 3);
        assert_eq!(outcome.trades[0].price, 100);
        assert_eq!(outcome.trades[0].quantity, 2);
        assert_eq!(outcome.trades[1].buy_order_id, 1);
        assert_eq!(outcome.trades[1].sell_order_id, 4);
        assert_eq!(outcome.trades[1].price, 100);
        assert_eq!(outcome.trades[1].quantity, 3);

        assert_eq!(pool.bids.len(), 1);
        assert!(pool.bids.iter().any(|order| order.order_id == 2 && order.remaining_quantity == 3));
        assert!(pool.bids.iter().all(|order| order.order_id != 1));

        assert_eq!(pool.asks.len(), 2);
        assert!(pool.asks.iter().any(|order| order.order_id == 4 && order.remaining_quantity == 1));
        assert!(pool.asks.iter().any(|order| order.order_id == 5 && order.remaining_quantity == 6));
    }

    #[test]
    fn cancel_order_removes_order_from_either_side() {
        let mut pool = CallAuctionPool::new(4);
        pool.add_order(make_order(1, OrderSide::Buy, 100, 2, 1)).unwrap();
        pool.add_order(make_order(2, OrderSide::Sell, 101, 2, 2)).unwrap();

        assert!(pool.cancel_order(&CancelOrder {
            product_id: 7,
            order_id: 1,
        }));
        assert!(pool.cancel_order(&CancelOrder {
            product_id: 7,
            order_id: 2,
        }));
        assert!(!pool.cancel_order(&CancelOrder {
            product_id: 7,
            order_id: 3,
        }));
        assert!(pool.bids.is_empty());
        assert!(pool.asks.is_empty());
    }

    #[test]
    fn rejects_market_orders_from_entering_call_auction_pool() {
        let mut pool = CallAuctionPool::new(4);

        let err = pool.add_order(make_order_with_price_type(
            1,
            OrderSide::Buy,
            PriceType::Market,
            0,
            5,
            1,
        ))
        .unwrap_err();

        assert_eq!(err.order.order_id, 1);
        assert_eq!(err.order.price_type, PriceType::Market);
        assert_eq!(
            err.source,
            CallAuctionPoolError::UnsupportedPriceType {
                price_type: PriceType::Market,
            }
        );
        assert!(pool.bids.is_empty());
        assert!(pool.asks.is_empty());
    }
}
