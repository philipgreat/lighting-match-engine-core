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

    /// Calculates the equilibrium price (the price that maximizes execution volume).
    /// Returns: Option<(Match Price, Total Executable Quantity)>
    pub fn calculate_match_price(&self) -> Option<(u64, u32)> {
        if self.bids.is_empty() || self.asks.is_empty() {
            return None;
        }

        // 1. Sorting Bids: Price descending (highest first), then time ascending (oldest first).
        let mut sorted_bids = self.bids.clone();
        sorted_bids.sort_by(|a, b| b.price.cmp(&a.price).then(a.submit_time.cmp(&b.submit_time)));

        // 2. Sorting Asks: Price ascending (lowest first), then time ascending (oldest first).
        let mut sorted_asks = self.asks.clone();
        sorted_asks.sort_by(|a, b| a.price.cmp(&b.price).then(a.submit_time.cmp(&b.submit_time)));

        // 3. Collect all unique price points from both sides as candidate prices.
        let mut candidate_prices: Vec<u64> = sorted_bids.iter().map(|o| o.price)
            .chain(sorted_asks.iter().map(|o| o.price))
            .collect();
        candidate_prices.sort_unstable();
        candidate_prices.dedup();

        let mut best_price = 0u64;
        let mut max_volume = 0u32;

        // 4. Iterate through candidate prices to find the one that maximizes volume.
        // For large datasets, a two-pointer approach or cumulative distribution is more efficient (O(N)).
        for &test_price in &candidate_prices {
            let mut cumulative_bid_vol = 0u32;
            let mut cumulative_ask_vol = 0u32;

            // Buyers willing to pay AT LEAST test_price
            for bid in &sorted_bids {
                if bid.price >= test_price {
                    cumulative_bid_vol += bid.quantity;
                } else {
                    break; // Optimization: further bids are lower price
                }
            }

            // Sellers willing to accept AT MOST test_price
            for ask in &sorted_asks {
                if ask.price <= test_price {
                    cumulative_ask_vol += ask.quantity;
                } else {
                    break; // Optimization: further asks are higher price
                }
            }

            let current_volume = min(cumulative_bid_vol, cumulative_ask_vol);

            // Update best price if a higher volume is found.
            if current_volume > max_volume {
                max_volume = current_volume;
                best_price = test_price;
            } 
            // Optional: Handle price ties (e.g., choose price closest to reference/last price)
            else if current_volume == max_volume && max_volume > 0 {
                // Example: Minimize market imbalance or stay closer to reference.
                // Current implementation keeps the first (lowest) candidate.
            }
        }

        if max_volume > 0 {
            Some((best_price, max_volume))
        } else {
            None
        }
    }

    /// Clears the pool and prepares the final match results.
    /// In a real engine, remaining quantities should be transferred back to the ContinuousOrderBook.
    pub fn execute_auction(&mut self, instance_tag: [u8; 16], product_id: u16, current_ts: u64) -> MatchResult {
        let mut result = MatchResult {
            order_execution_list: Vec::new(),
            start_time: current_ts,
            end_time: 0,
        };

        if let Some((match_price, total_volume)) = self.calculate_match_price() {
            // Logic to generate OrderExecution events would go here.
            // This involves iterating through sorted_bids and sorted_asks 
            // and deducting 'total_volume' from the top of the queues.
        }

        result.end_time = current_ts; // Set closing timestamp
        result
    }

    /// Resets the pool after the auction period ends.
    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }

}