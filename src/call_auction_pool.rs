use crate::data_types::*; 
use std::cmp::{max, min};

impl CallAuctionPool {
    /// Creates a new, empty Call Auction Pool.
    pub fn new(init_size:usize) -> Self {
        Self {
            bids: Vec::with_capacity(init_size),
            asks: Vec::with_capacity(init_size),
        }
    }

    /// Adds an incoming order to the appropriate side of the pool.
    pub fn add_order(&mut self, order: Order) {
        match order.order_type {
            ORDER_TYPE_BUY | ORDER_TYPE_MOCK_BUY => self.bids.push(order),
            ORDER_TYPE_SELL | ORDER_TYPE_MOCK_SELL => self.asks.push(order),
            _ => {} // Ignore unknown types
        }
    }

/// Optimized Equilibrium Price Calculation using Two-Pointer Sweep-Line.
    /// Complexity: O(N log N) due to sorting, O(N) for scanning.
pub fn calculate_match_price_final(&self, price_tick: u64) -> Option<(u64, u32)> {
        if self.bids.is_empty() || self.asks.is_empty() || price_tick == 0 {
            return None;
        }

        // 1. 收集所有原始委托价格并排序（不考虑 tick）
        let mut raw_prices: Vec<u64> = self.bids.iter().map(|o| o.price)
            .chain(self.asks.iter().map(|o| o.price))
            .collect();
        raw_prices.sort_unstable();
        raw_prices.dedup();

        // 2. 将这些价格映射到最近的合规 tick
        // 我们需要检查：每个委托价对应的当前 tick，以及它的前一个和后一个 tick
        let mut critical_ticks = Vec::new();
        for p in raw_prices {
            let base = (p / price_tick) * price_tick;
            critical_ticks.push(base);
            critical_ticks.push(base + price_tick);
            if base >= price_tick {
                critical_ticks.push(base - price_tick);
            }
        }
        critical_ticks.sort_unstable();
        critical_ticks.dedup();

        // 3. 准备双指针扫描所需的排序数组
        let mut sorted_bids = self.bids.clone();
        sorted_bids.sort_by(|a, b| b.price.cmp(&a.price)); // 高到低

        let mut sorted_asks = self.asks.clone();
        sorted_asks.sort_by(|a, b| a.price.cmp(&b.price)); // 低到高

        // 4. 双指针扫描逻辑
        let mut best_price = 0u64;
        let mut max_volume = 0u32;
        let mut min_imbalance = u32::MAX;

        // 初始化累计成交量
        let mut total_bid_vol: u32 = sorted_bids.iter().map(|o| o.quantity).sum();
        let mut total_ask_vol: u32 = 0;
        let mut bid_idx = 0; // 指向 sorted_bids 中价格 < test_price 的第一个订单
        let mut ask_idx = 0; // 指向 sorted_asks 中价格 <= test_price 的最后一个订单之后

        // 注意：由于 critical_ticks 是递增的
        // 我们需要高效更新 total_bid_vol (价格 >= test_price 的和)
        // 和 total_ask_vol (价格 <= test_price 的和)

        // 修正排序后的索引位置以适应 total_bid_vol 的定义
        // 我们先让 bid_idx 指向数组末尾，随着 test_price 升高向左移动
        let mut bid_ptr = sorted_bids.len(); 

        for &test_price in &critical_ticks {
            // 移除那些价格已经低于当前 test_price 的买单
            while bid_ptr > 0 && sorted_bids[bid_ptr - 1].price < test_price {
                total_bid_vol -= sorted_bids[bid_ptr - 1].quantity;
                bid_ptr -= 1;
            }
            // 加入那些价格已经符合当前 test_price 的卖单
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
            } else if current_vol == max_volume && max_volume > 0 {
                if imbalance < min_imbalance {
                    best_price = test_price;
                    min_imbalance = imbalance;
                }
            }
        }

        if max_volume > 0 { Some((best_price, max_volume)) } else { None }
    }

    /// Handles the actual execution of the auction, generating MatchResults.
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

        // 1. Calculate the price and the total volume to match
        let (match_price, mut total_volume_to_match) = match self.calculate_match_price_final(price_tick) {
            Some(res) => res,
            None => return match_result, // Nothing to match
        };

        // 2. Prepare candidate orders
        // Buy Side: Orders with price >= match_price, sorted by Price desc, Time asc.
        let mut eligible_bids: Vec<Order> = self.bids.drain(..)
            .filter(|o| o.price >= match_price)
            .collect();
        eligible_bids.sort_by(|a, b| b.price.cmp(&a.price).then(a.submit_time.cmp(&b.submit_time)));

        // Sell Side: Orders with price <= match_price, sorted by Price asc, Time asc.
        let mut eligible_asks: Vec<Order> = self.asks.drain(..)
            .filter(|o| o.price <= match_price)
            .collect();
        eligible_asks.sort_by(|a, b| a.price.cmp(&b.price).then(a.submit_time.cmp(&b.submit_time)));

        // 3. Bilateral Matching
        let mut b_idx = 0;
        let mut s_idx = 0;

        while b_idx < eligible_bids.len() && s_idx < eligible_asks.len() && total_volume_to_match > 0 {
            let bid = &mut eligible_bids[b_idx];
            let ask = &mut eligible_asks[s_idx];

            let match_qty = min(bid.quantity, min(ask.quantity, total_volume_to_match));

            if match_qty > 0 {
                let execution = OrderExecution {
                    instance_tag,
                    product_id,
                    buy_order_id: bid.order_id,
                    sell_order_id: ask.order_id,
                    price: match_price,
                    quantity: match_qty,
                    trade_time_network: 0, // Set by network layer
                    internal_match_time: 0, // Latency metric
                    is_mocked_result: bid.is_mocked_order() || ask.is_mocked_order(),
                };

                match_result.order_execution_list.push(execution);
                
                bid.quantity -= match_qty;
                ask.quantity -= match_qty;
                total_volume_to_match -= match_qty;
            }

            // Move pointers if orders are fully exhausted
            if bid.quantity == 0 { b_idx += 1; }
            if ask.quantity == 0 { s_idx += 1; }
        }

        // 4. Clean up: Return unexecuted portions of orders back to the pool 
        // or prepare them for the Continuous Trading session.
        self.bids.extend(eligible_bids.into_iter().filter(|o| o.quantity > 0));
        self.asks.extend(eligible_asks.into_iter().filter(|o| o.quantity > 0));

        match_result.end_time = 0; // Update with actual end timestamp if needed
        match_result
    }

    /// Resets the pool after the auction period ends.
    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }
    pub fn cancel_order(&mut self, cancel: &CancelOrder) -> bool {
        let mut removed = false;

        // Check the Buy side
        if let Some(pos) = self.bids.iter().position(|o| o.order_id == cancel.order_id) {
            self.bids.remove(pos);
            removed = true;
        } 
        // If not found in bids, check the Sell side
        else if let Some(pos) = self.asks.iter().position(|o| o.order_id == cancel.order_id) {
            self.asks.remove(pos);
            removed = true;
        }

        removed
    }

}