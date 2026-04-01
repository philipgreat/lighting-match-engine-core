use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::engine_core::types::{
    MatchResult, Order, OrderExecution, OrdersBucket, SparseOrderBook, ORDER_PRICE_TYPE_LIMIT,
};

impl SparseOrderBook {
    pub fn new(tick: u64, base_price: u64, _max_levels: usize, trade_cap: usize) -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_map: BTreeMap::new(),
            total_bid_volume: 0,
            total_ask_volume: 0,
            match_result: MatchResult::new(trade_cap),
            tick,
            base_price,
        }
    }

    pub fn fuel_order(&mut self, order: Order) {
        self.add_resting_order(order);
    }

    pub fn match_order(&mut self, mut order: Order) {
        self.match_result.order_execution_list.clear();

        if order.is_buy() {
            self.match_buy(&mut order);
        } else {
            self.match_sell(&mut order);
        }

        if order.quantity > 0 && order.price_type == ORDER_PRICE_TYPE_LIMIT {
            self.add_resting_order(order);
        }
    }

    pub fn cancel_order(&mut self, order_id: u64) -> bool {
        let (is_buy, price) = match self.order_map.remove(&order_id) {
            Some(value) => value,
            None => return false,
        };

        let ladder = if is_buy {
            &mut self.bids
        } else {
            &mut self.asks
        };

        if let Some(bucket) = ladder.get_mut(&price) {
            if let Some(pos) = bucket.orders.iter().position(|order| order.order_id == order_id) {
                let removed = bucket.orders.remove(pos).unwrap();
                if is_buy {
                    self.total_bid_volume -= removed.quantity;
                } else {
                    self.total_ask_volume -= removed.quantity;
                }

                if bucket.orders.is_empty() {
                    ladder.remove(&price);
                }
                return true;
            }
        }

        false
    }

    fn add_resting_order(&mut self, order: Order) {
        let is_buy = order.is_buy();
        let price = order.price;

        self.order_map.insert(order.order_id, (is_buy, price));

        let bucket = if is_buy {
            self.total_bid_volume += order.quantity;
            self.bids.entry(price).or_default()
        } else {
            self.total_ask_volume += order.quantity;
            self.asks.entry(price).or_default()
        };

        bucket.orders.push_back(order);
    }

    fn match_buy(&mut self, order: &mut Order) {
        let mut empty_prices = Vec::new();

        for (&price, bucket) in self.asks.iter_mut() {
            if order.quantity == 0
                || (order.price_type == ORDER_PRICE_TYPE_LIMIT && order.price < price)
            {
                break;
            }

            Self::execute_matching(
                order,
                bucket,
                true,
                &mut self.match_result,
                &mut self.order_map,
                &mut self.total_ask_volume,
                &mut self.total_bid_volume,
            );

            if bucket.orders.is_empty() {
                empty_prices.push(price);
            }
        }

        for price in empty_prices {
            self.asks.remove(&price);
        }
    }

    fn match_sell(&mut self, order: &mut Order) {
        let mut empty_prices = Vec::new();

        for (&price, bucket) in self.bids.iter_mut().rev() {
            if order.quantity == 0
                || (order.price_type == ORDER_PRICE_TYPE_LIMIT && order.price > price)
            {
                break;
            }

            Self::execute_matching(
                order,
                bucket,
                false,
                &mut self.match_result,
                &mut self.order_map,
                &mut self.total_ask_volume,
                &mut self.total_bid_volume,
            );

            if bucket.orders.is_empty() {
                empty_prices.push(price);
            }
        }

        for price in empty_prices {
            self.bids.remove(&price);
        }
    }

    fn execute_matching(
        taker: &mut Order,
        bucket: &mut OrdersBucket,
        taker_is_buy: bool,
        match_result: &mut MatchResult,
        order_map: &mut BTreeMap<u64, (bool, u64)>,
        total_ask_vol: &mut u32,
        total_bid_vol: &mut u32,
    ) {
        while taker.quantity > 0 && !bucket.orders.is_empty() {
            let resting = bucket.orders.front_mut().unwrap();
            let qty = taker.quantity.min(resting.quantity);

            taker.quantity -= qty;
            resting.quantity -= qty;

            if taker_is_buy {
                *total_ask_vol -= qty;
            } else {
                *total_bid_vol -= qty;
            }

            match_result.add_order_execution(OrderExecution {
                instance_tag: [0; 16],
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
                trade_time_network: 0,
                internal_match_time: 0,
                is_mocked_result: taker.is_mocked_order(),
            });

            if resting.quantity == 0 {
                let filled = bucket.orders.pop_front().unwrap();
                order_map.remove(&filled.order_id);
            }
        }
    }
}
