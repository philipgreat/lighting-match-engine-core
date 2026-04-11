use crate::types::*;
use std::cmp::Ordering;
use std::cmp::min;

impl CallAuctionPool {
    pub fn new(init_size: usize) -> Self {
        Self {
            bids: Vec::with_capacity(init_size),
            asks: Vec::with_capacity(init_size),
        }
    }

    pub fn add_order(&mut self, order: OrderRequest) -> Result<(), CallAuctionPoolError> {
        if !order.is_limit() {
            return Err(CallAuctionPoolError::UnsupportedPriceType {
                price_type: order.price_type,
            });
        }

        let resting = order.into_resting_order();
        if resting.is_buy() {
            self.bids.push(resting);
        } else if resting.is_sell() {
            self.asks.push(resting);
        }

        Ok(())
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
        let mut has_candidate = false;

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
        ));

        assert_eq!(
            err,
            Err(CallAuctionPoolError::UnsupportedPriceType {
                price_type: PriceType::Market,
            })
        );
        assert!(pool.bids.is_empty());
        assert!(pool.asks.is_empty());
    }
}
