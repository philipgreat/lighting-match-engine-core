use ahash::AHashMap;
use std::collections::BTreeMap;

use crate::timer::HighResolutionTimer;
use crate::types::*;

impl SparseOrderBook {
    fn prune_bucket_front(
        bucket: &mut OrdersBucket,
        order_map: &mut AHashMap<u64, (bool, u64)>,
    ) {
        while matches!(bucket.orders.front(), Some(order) if !order.is_active()) {
            let removed = bucket.orders.pop_front().unwrap();
            order_map.remove(&removed.order_id);
        }
    }

    pub fn new(
        tick: u64,
        base_price: u64,
        _max_levels: usize,
        trade_cap: usize,
    ) -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_map: AHashMap::with_capacity(1024),
            empty_bid_prices_buf: Vec::with_capacity(64),
            empty_ask_prices_buf: Vec::with_capacity(64),
            total_bid_volume: 0,
            total_ask_volume: 0,
            last_outcome: MatchOutcome::new(trade_cap),
            tick,
            base_price,
            timer: HighResolutionTimer::start(),
        }
    }

    pub fn seed_order(&mut self, order: OrderRequest) -> Result<(), OrderSubmitError> {
        self.add_resting_order(order.into_resting_order());
        Ok(())
    }

    pub fn match_order(&mut self, incoming: OrderRequest) -> Result<(), OrderSubmitError> {
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

    pub fn cancel_order(&mut self, order_id: u64) -> bool {
        let (is_buy, price) = match self.order_map.remove(&order_id) {
            Some(v) => v,
            None => return false,
        };

        let ladder = if is_buy { &mut self.bids } else { &mut self.asks };

        if let Some(bucket) = ladder.get_mut(&price) {
            let is_front = bucket
                .orders
                .front()
                .map(|order| order.order_id == order_id)
                .unwrap_or(false);

            if let Some(removed) = bucket.orders.iter_mut().find(|o| o.order_id == order_id) {
                if is_buy {
                    self.total_bid_volume -= removed.remaining_quantity;
                } else {
                    self.total_ask_volume -= removed.remaining_quantity;
                }
                removed.remaining_quantity = 0;
                removed.is_cancelled = true;

                if is_front {
                    Self::prune_bucket_front(bucket, &mut self.order_map);
                }

                if bucket.orders.is_empty() {
                    ladder.remove(&price);
                }
                return true;
            }
        }
        false
    }

    fn add_resting_order(&mut self, order: RestingOrder) {
        let is_buy = order.is_buy();
        let price = order.price;

        self.order_map.insert(order.order_id, (is_buy, price));

        let bucket = if is_buy {
            self.total_bid_volume += order.remaining_quantity;
            self.bids.entry(price).or_default()
        } else {
            self.total_ask_volume += order.remaining_quantity;
            self.asks.entry(price).or_default()
        };

        bucket.orders.push_back(order);
    }

    fn match_buy(&mut self, taker: &mut RestingOrder) {
        self.empty_ask_prices_buf.clear();

        for (&price, bucket) in self.asks.iter_mut() {
            Self::prune_bucket_front(bucket, &mut self.order_map);
            if taker.remaining_quantity == 0 || (taker.is_limit() && taker.price < price) {
                break;
            }

            Self::execute_matching(
                taker,
                bucket,
                true,
                &mut self.last_outcome,
                &mut self.order_map,
                &mut self.total_ask_volume,
                &mut self.total_bid_volume,
            );

            if bucket.orders.is_empty() {
                self.empty_ask_prices_buf.push(price);
            }
        }

        for price in self.empty_ask_prices_buf.drain(..) {
            self.asks.remove(&price);
        }
    }

    fn match_sell(&mut self, taker: &mut RestingOrder) {
        self.empty_bid_prices_buf.clear();

        for (&price, bucket) in self.bids.iter_mut().rev() {
            Self::prune_bucket_front(bucket, &mut self.order_map);
            if taker.remaining_quantity == 0 || (taker.is_limit() && taker.price > price) {
                break;
            }

            Self::execute_matching(
                taker,
                bucket,
                false,
                &mut self.last_outcome,
                &mut self.order_map,
                &mut self.total_ask_volume,
                &mut self.total_bid_volume,
            );

            if bucket.orders.is_empty() {
                self.empty_bid_prices_buf.push(price);
            }
        }

        for price in self.empty_bid_prices_buf.drain(..) {
            self.bids.remove(&price);
        }
    }

    fn execute_matching(
        taker: &mut RestingOrder,
        bucket: &mut OrdersBucket,
        taker_is_buy: bool,
        outcome: &mut MatchOutcome,
        order_map: &mut AHashMap<u64, (bool, u64)>,
        total_ask_volume: &mut u32,
        total_bid_volume: &mut u32,
    ) {
        while taker.remaining_quantity > 0 && !bucket.orders.is_empty() {
            let resting = bucket.orders.front_mut().unwrap();
            let qty = taker.remaining_quantity.min(resting.remaining_quantity);

            taker.remaining_quantity -= qty;
            resting.remaining_quantity -= qty;

            if taker_is_buy {
                *total_ask_volume -= qty;
            } else {
                *total_bid_volume -= qty;
            }

            outcome.add_trade(Trade {
                product_id: taker.product_id,
                buy_order_id: if taker_is_buy {
                    taker.order_id
                } else {
                    resting.order_id
                },
                sell_order_id: if taker_is_buy {
                    resting.order_id
                } else {
                    taker.order_id
                },
                price: resting.price,
                quantity: qty,
                involves_mock_order: taker.is_mocked_order() || resting.is_mocked_order(),
            });

            if resting.remaining_quantity == 0 {
                let removed = bucket.orders.pop_front().unwrap();
                order_map.remove(&removed.order_id);
            }
        }
    }
}
