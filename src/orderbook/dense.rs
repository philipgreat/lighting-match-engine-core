use ahash::AHashMap;

use crate::timer::HighResolutionTimer;
use crate::types::*;

impl DenseOrderBook {
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
            total_bid_volume: 0,
            total_ask_volume: 0,
            last_outcome: MatchOutcome::new(trade_cap),
            timer: HighResolutionTimer::start(),
        }
    }

    #[inline(always)]
    fn price_to_index(&self, price: u64) -> usize {
        ((price - self.base_price) / self.tick) as usize
    }

    fn highest_supported_price(&self) -> u64 {
        self.base_price + self.tick * (self.levels as u64 - 1)
    }

    fn validate_price_range(&self, price: u64) -> Result<(), OrderBookError> {
        let min_price = self.base_price;
        let max_price = self.highest_supported_price();

        if price < min_price || price > max_price {
            return Err(OrderBookError::PriceOutOfRange {
                price,
                min_price,
                max_price,
            });
        }

        Ok(())
    }

    fn validate_price_on_tick(&self, price: u64) -> Result<(), OrderBookError> {
        if (price - self.base_price) % self.tick != 0 {
            return Err(OrderBookError::PriceNotOnTick {
                price,
                base_price: self.base_price,
                tick: self.tick,
            });
        }

        Ok(())
    }

    #[inline(always)]
    fn has_asks(&self) -> bool {
        self.best_ask < self.levels as isize
    }

    #[inline(always)]
    fn has_bids(&self) -> bool {
        self.best_bid >= 0
    }

    fn advance_best_ask(&mut self) {
        while self.has_asks() {
            let idx = self.best_ask as usize;
            if !self.asks[idx].orders.is_empty() {
                break;
            }
            self.best_ask += 1;
        }
    }

    fn retreat_best_bid(&mut self) {
        while self.has_bids() {
            let idx = self.best_bid as usize;
            if !self.bids[idx].orders.is_empty() {
                break;
            }
            self.best_bid -= 1;
        }
    }

    fn add_resting_order(&mut self, order: RestingOrder) {
        let idx = self.price_to_index(order.price);

        if order.is_buy() {
            self.total_bid_volume += order.remaining_quantity;
            self.best_bid = self.best_bid.max(idx as isize);
            self.order_map.insert(order.order_id, (true, idx));
            self.bids[idx].orders.push_back(order);
        } else {
            self.total_ask_volume += order.remaining_quantity;
            self.best_ask = self.best_ask.min(idx as isize);
            self.order_map.insert(order.order_id, (false, idx));
            self.asks[idx].orders.push_back(order);
        }
    }

    pub fn seed_order(&mut self, order: OrderRequest) -> Result<(), OrderBookError> {
        self.validate_price_range(order.price)?;
        self.validate_price_on_tick(order.price)?;
        self.add_resting_order(order.into_resting_order());
        Ok(())
    }

    pub fn match_order(&mut self, incoming: OrderRequest) -> Result<(), OrderBookError> {
        self.validate_price_range(incoming.price)?;
        self.validate_price_on_tick(incoming.price)?;
        let mut taker = incoming.into_resting_order();
        self.last_outcome.trades.clear();
#[cfg(feature = "match-timing")]
        {
            self.last_outcome.start_time = self.timer.ns() as u64;
        }

        if taker.is_buy() {
            self.match_buy(&mut taker);
        } else {
            self.match_sell(&mut taker);
        }

        if taker.remaining_quantity > 0 && taker.is_limit() {
            self.add_resting_order(taker);
        }

#[cfg(feature = "match-timing")]
        {
            self.last_outcome.end_time = self.timer.ns() as u64;
        }
        Ok(())
    }

    fn match_buy(&mut self, taker: &mut RestingOrder) {
        self.advance_best_ask();

        while taker.remaining_quantity > 0 && self.has_asks() {
            let idx = self.best_ask as usize;
            let bucket = &mut self.asks[idx];

            if bucket.orders.is_empty() {
                self.best_ask += 1;
                continue;
            }

            let resting = bucket.orders.front_mut().unwrap();

            if taker.is_limit() && taker.price < resting.price {
                break;
            }

            let qty = taker.remaining_quantity.min(resting.remaining_quantity);
            taker.remaining_quantity -= qty;
            resting.remaining_quantity -= qty;
            self.total_ask_volume -= qty;

            self.last_outcome.add_trade(Trade {
                product_id: taker.product_id,
                buy_order_id: taker.order_id,
                sell_order_id: resting.order_id,
                price: resting.price,
                quantity: qty,
                involves_mock_order: taker.is_mocked_order() || resting.is_mocked_order(),
            });

            if resting.remaining_quantity == 0 {
                let removed = bucket.orders.pop_front().unwrap();
                self.order_map.remove(&removed.order_id);
                if bucket.orders.is_empty() {
                    self.best_ask += 1;
                    self.advance_best_ask();
                }
            }
        }
    }

    fn match_sell(&mut self, taker: &mut RestingOrder) {
        self.retreat_best_bid();

        while taker.remaining_quantity > 0 && self.has_bids() {
            let idx = self.best_bid as usize;
            let bucket = &mut self.bids[idx];

            if bucket.orders.is_empty() {
                self.best_bid -= 1;
                continue;
            }

            let resting = bucket.orders.front_mut().unwrap();

            if taker.is_limit() && taker.price > resting.price {
                break;
            }

            let qty = taker.remaining_quantity.min(resting.remaining_quantity);
            taker.remaining_quantity -= qty;
            resting.remaining_quantity -= qty;
            self.total_bid_volume -= qty;

            self.last_outcome.add_trade(Trade {
                product_id: taker.product_id,
                buy_order_id: resting.order_id,
                sell_order_id: taker.order_id,
                price: resting.price,
                quantity: qty,
                involves_mock_order: taker.is_mocked_order() || resting.is_mocked_order(),
            });

            if resting.remaining_quantity == 0 {
                let removed = bucket.orders.pop_front().unwrap();
                self.order_map.remove(&removed.order_id);
                if bucket.orders.is_empty() {
                    self.best_bid -= 1;
                    self.retreat_best_bid();
                }
            }
        }
    }

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
            let removed = bucket.orders.remove(pos).unwrap();
            if is_buy {
                self.total_bid_volume -= removed.remaining_quantity;
                if bucket.orders.is_empty() && self.best_bid == idx as isize {
                    self.best_bid -= 1;
                    self.retreat_best_bid();
                }
            } else {
                self.total_ask_volume -= removed.remaining_quantity;
                if bucket.orders.is_empty() && self.best_ask == idx as isize {
                    self.best_ask += 1;
                    self.advance_best_ask();
                }
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{OrderFlags, OrderRequest, OrderSide, PriceType};

    fn order(order_id: u64, side: OrderSide, price: u64, quantity: u32) -> OrderRequest {
        OrderRequest {
            product_id: 1,
            side,
            price_type: PriceType::Limit,
            flags: OrderFlags::default(),
            quantity,
            order_id,
            price,
            submit_time: 0,
            expire_time: 0,
            _padding: [0u8; 24],
        }
    }

        #[test]
    fn sell_matches_existing_bid_when_asks_are_empty() {
        let mut book = DenseOrderBook::new(1, 1, 16, 8);
        book.seed_order(order(1, OrderSide::Buy, 10, 5)).unwrap();

        book.match_order(order(2, OrderSide::Sell, 9, 3)).unwrap();

        assert_eq!(book.last_outcome.total_count(), 1);
        let trade = &book.last_outcome.trades[0];
        assert_eq!(trade.buy_order_id, 1);
        assert_eq!(trade.sell_order_id, 2);
        assert_eq!(trade.price, 10);
        assert_eq!(trade.quantity, 3);
        assert_eq!(book.total_bid_volume, 2);
        assert_eq!(book.total_ask_volume, 0);
    }

    #[test]
    fn buy_matches_existing_ask_when_bids_are_empty() {
        let mut book = DenseOrderBook::new(1, 1, 16, 8);
        book.seed_order(order(1, OrderSide::Sell, 10, 5)).unwrap();

        book.match_order(order(2, OrderSide::Buy, 11, 3)).unwrap();

        assert_eq!(book.last_outcome.total_count(), 1);
        let trade = &book.last_outcome.trades[0];
        assert_eq!(trade.buy_order_id, 2);
        assert_eq!(trade.sell_order_id, 1);
        assert_eq!(trade.price, 10);
        assert_eq!(trade.quantity, 3);
        assert_eq!(book.total_bid_volume, 0);
        assert_eq!(book.total_ask_volume, 2);
    }

    #[test]
    fn rejects_price_outside_dense_book_range() {
        let mut book = DenseOrderBook::new(10, 100, 4, 8);

        let err = book.seed_order(order(1, OrderSide::Buy, 50, 1)).unwrap_err();
        assert_eq!(
            err,
            OrderBookError::PriceOutOfRange {
                price: 50,
                min_price: 100,
                max_price: 130,
            }
        );
    }

    #[test]
    fn rejects_price_not_aligned_to_tick() {
        let mut book = DenseOrderBook::new(10, 100, 4, 8);

        let err = book.seed_order(order(1, OrderSide::Buy, 105, 1)).unwrap_err();
        assert_eq!(
            err,
            OrderBookError::PriceNotOnTick {
                price: 105,
                base_price: 100,
                tick: 10,
            }
        );
    }
}
