// ================================
// sparse_order_book.rs
// ================================

use ahash::AHashMap;
use std::collections::{BTreeMap, VecDeque};

use crate::data_types::*;
use crate::high_resolution_timer::HighResolutionTimer;

/// 稀疏价格阶梯订单簿
/// 优点：内存占用极低（仅为活跃档位分配空间），支持任意价格范围。
/// 缺点：性能略低于 Dense 数组版本（O(log N) vs O(1)）。


impl SparseOrderBook {
    pub fn new(
        tick: u64,
        base_price: u64,
        _max_levels: usize, // 稀疏模式下此参数仅为保持接口一致
        trade_cap: usize,
    ) -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_map: AHashMap::with_capacity(1024),
            total_bid_volume: 0,
            total_ask_volume: 0,
            match_result: MatchResult::new(trade_cap),
            tick,
            base_price,
            timer: HighResolutionTimer::start(),
        }
    }

    // ----------------------------
    // Public API
    // ----------------------------
    
    pub fn fuel_order(&mut self, order: Order) {
        self.add_resting_order(order);
    }

    pub fn match_order(&mut self, mut order: Order) {
        self.match_result.order_execution_list.clear();
        self.match_result.start_time = self.timer.ns() as u64;

        if order.is_buy() {
            self.match_buy(&mut order);
        } else {
            self.match_sell(&mut order);
        }

        // 如果是限价单且未完全成交，进入订单簿
        if order.quantity > 0 && order.price_type == ORDER_PRICE_TYPE_LIMIT {
            self.add_resting_order(order);
        }
        
        self.match_result.end_time = self.timer.ns() as u64;
    }

    pub fn cancel_order(&mut self, order_id: u64) -> bool {
        let (is_buy, price) = match self.order_map.remove(&order_id) {
            Some(v) => v,
            None => return false,
        };

        let ladder = if is_buy { &mut self.bids } else { &mut self.asks };
        
        if let Some(bucket) = ladder.get_mut(&price) {
            if let Some(pos) = bucket.orders.iter().position(|o| o.order_id == order_id) {
                let o = bucket.orders.remove(pos).unwrap();
                if is_buy {
                    self.total_bid_volume -= o.quantity;
                } else {
                    self.total_ask_volume -= o.quantity;
                }
                
                // 如果该价格档位空了，移除它以节省内存
                if bucket.orders.is_empty() {
                    ladder.remove(&price);
                }
                return true;
            }
        }
        false
    }

    // ----------------------------
    // Private Logic
    // ----------------------------

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
            if order.quantity == 0 || (order.price_type == ORDER_PRICE_TYPE_LIMIT && order.price < price) {
                break;
            }

            // 修复点：调用关联函数，只传入需要的字段引用
            Self::execute_matching(
                order,
                bucket,
                true,
                &mut self.match_result,
                &mut self.order_map,
                &mut self.total_ask_volume, // 传入需要修改的量
                &mut self.total_bid_volume
            );
            
            if bucket.orders.is_empty() {
                empty_prices.push(price);
            }
        }

        for p in empty_prices { self.asks.remove(&p); }
    }

    fn match_sell(&mut self, order: &mut Order) {
        let mut empty_prices = Vec::new();
        
        for (&price, bucket) in self.bids.iter_mut().rev() {
            if order.quantity == 0 || (order.price_type == ORDER_PRICE_TYPE_LIMIT && order.price > price) {
                break;
            }

            Self::execute_matching(
                order,
                bucket,
                false,
                &mut self.match_result,
                &mut self.order_map,
                &mut self.total_ask_volume,
                &mut self.total_bid_volume
            );
            
            if bucket.orders.is_empty() {
                empty_prices.push(price);
            }
        }

        for p in empty_prices { self.bids.remove(&p); }
    }

    // 修复点：改为关联函数，不带 &mut self
    fn execute_matching(
        taker: &mut Order, 
        bucket: &mut OrdersBucket, 
        taker_is_buy: bool,
        match_result: &mut MatchResult,
        order_map: &mut AHashMap<u64, (bool, u64)>,
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

            match_result.order_execution_list.push(OrderExecution {
                instance_tag: [0; 16],
                product_id: taker.product_id,
                buy_order_id: if taker_is_buy { taker.order_id } else { resting.order_id },
                sell_order_id: if taker_is_buy { resting.order_id } else { taker.order_id },
                price: resting.price,
                quantity: qty,
                trade_time_network: 0,
                internal_match_time: 0,
                is_mocked_result: taker.is_mocked_order(),
            });

            if resting.quantity == 0 {
                let o = bucket.orders.pop_front().unwrap();
                order_map.remove(&o.order_id);
            }
        }
    }
    
}


