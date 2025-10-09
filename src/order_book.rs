use crate::high_resolution_timer::HighResultionCounter;
use crate::{data_types::ORDER_TYPE_MOCK_BUY, date_time_tool::current_timestamp};
use tokio::sync::RwLock;
// Assuming these are defined in data_types.rs
// NOTE: In a real Rust project, you'd replace 'crate::data_types' with the actual path.
use crate::data_types::{
    MatchResult, MockMatchResult, ORDER_PRICE_TYPE_LIMIT, ORDER_PRICE_TYPE_MARKET, ORDER_TYPE_BUY,
    ORDER_TYPE_MOCK_SELL, ORDER_TYPE_SELL, Order, OrderBook, OrderIndex,
};

// --- Helper Structs and Trait ---

/// A temporary structure to hold the information of the resting order involved in a match
/// so that it can be processed in post_match (e.g., deletion or quantity update).
#[derive(Debug, Clone, Copy)]
pub struct MatchedRestingOrder {
    pub order_index: OrderIndex, // Index in the bids or asks vector
    pub matched_quantity: u32,   // Quantity matched from this resting order
    pub is_buy: bool,            // true if the order is from the bids array (buy side)
}

/// The core trait for sending match results (trade signals) to an external system.
/// The implementation will be external to this file.
pub trait ResultSender: Send + Sync {
    // Added Send + Sync for concurrent use
    async fn send_result(&self, result: MatchResult);
}

// --- OrderBook Definition ---

// pub struct OrderBook {
//     // Orders on the buy side (bids)
//     pub bids: RwLock<Vec<Order>>,
//     // Orders on the sell side (asks)
//     pub asks: RwLock<Vec<Order>>,

//     // Indices of the top N best-priced bid orders (price then time priority)
//     pub top_bids_index: RwLock<Vec<OrderIndex>>,
//     // Indices of the top N best-priced ask orders (price then time priority)
//     pub top_asks_index: RwLock<Vec<OrderIndex>>,

//     // Initial capacity for bids and asks vectors
//     pub init_order_book_size: u32,
//     // Max number of best-priced indices to keep in top_bids_index and top_asks_index
//     pub init_top_index_size: u32,
// }

impl OrderBook {
    /// Constructs a new OrderBook with specified initial capacities.
    pub fn new(instance_tag: [u8; 8], initial_book_size: u32, initial_top_size: u32) -> Self {
        OrderBook {
            instance_tag: instance_tag,
            bids: RwLock::new(Vec::with_capacity(initial_book_size as usize)),
            asks: RwLock::new(Vec::with_capacity(initial_book_size as usize)),

            top_bids_index: RwLock::new(Vec::with_capacity(initial_top_size as usize)),
            top_asks_index: RwLock::new(Vec::with_capacity(initial_top_size as usize)),

            init_order_book_size: initial_book_size,
            init_top_index_size: initial_top_size,
        }
    }

    // --- Phase 1: Fuel Order (Adding orders) ---

    /// Adds an order to the order book (bids or asks).
    pub async fn fuel_order(&self, order: Order) {
        if order.order_type == ORDER_TYPE_BUY {
            // Acquire a write lock asynchronously
            let mut bids = self.bids.write().await;
            // In a real system, insert the order while maintaining price/time priority.
            bids.push(order);
        } else if order.order_type == ORDER_TYPE_SELL {
            // Acquire a write lock asynchronously
            let mut asks = self.asks.write().await;
            // In a real system, insert the order while maintaining price/time priority.
            asks.push(order);
        }
    }

    // --- Phase 2: Index Preparation ---

    /// Finds and stores the indices of the best bid orders. (async)
    // --- Phase 2: Index Preparation ---

    /// Finds and stores the indices of the best bid orders based on Price (desc) then Time (asc). (async)
    async fn prepare_bids_index(&self) {
        // 1. Acquire read lock for bids
        let bids_guard = self.bids.read().await;

        // 2. Create a list of (index, price, submit_time) for sorting
        let mut indexed_bids: Vec<(OrderIndex, u64, u64)> = bids_guard
            .iter()
            .enumerate()
            // Map the order to its index, price, and submission time
            .map(|(i, order)| (i as OrderIndex, order.price, order.submit_time))
            .collect();

        // 3. Sort the list: Price DESC (b.1.cmp(a.1)) then Time ASC (a.2.cmp(b.2))
        // Bids: Higher price is better, then older time is better.
        indexed_bids.sort_by(|a, b| {
            // Compare Price (Descending)
            b.1.cmp(&a.1)
                // If prices are equal, compare Time (Ascending)
                .then_with(|| a.2.cmp(&b.2))
        });

        // 4. Acquire write lock for top_bids_index
        let mut top_bids_index_guard = self.top_bids_index.write().await;
        top_bids_index_guard.clear();

        // 5. Take the first N indices (top orders)
        let max_size = self.init_top_index_size as usize;
        for (index, _, _) in indexed_bids.into_iter().take(max_size) {
            top_bids_index_guard.push(index);
        }

        // Lock guards are dropped here automatically.
    }

    /// Finds and stores the indices of the best ask orders based on Price (asc) then Time (asc). (async)
    async fn prepare_asks_index(&self) {
        // 1. Acquire read lock for asks
        let asks_guard = self.asks.read().await;

        // 2. Create a list of (index, price, submit_time) for sorting
        let mut indexed_asks: Vec<(OrderIndex, u64, u64)> = asks_guard
            .iter()
            .enumerate()
            // Map the order to its index, price, and submission time
            .map(|(i, order)| (i as OrderIndex, order.price, order.submit_time))
            .collect();

        // 3. Sort the list: Price ASC (a.1.cmp(b.1)) then Time ASC (a.2.cmp(b.2))
        // Asks: Lower price is better, then older time is better.
        indexed_asks.sort_by(|a, b| {
            // Compare Price (Ascending)
            a.1.cmp(&b.1)
                // If prices are equal, compare Time (Ascending)
                .then_with(|| a.2.cmp(&b.2))
        });

        // 4. Acquire write lock for top_asks_index
        let mut top_asks_index_guard = self.top_asks_index.write().await;
        top_asks_index_guard.clear();

        // 5. Take the first N indices (top orders)
        let max_size = self.init_top_index_size as usize;
        for (index, _, _) in indexed_asks.into_iter().take(max_size) {
            top_asks_index_guard.push(index);
        }

        // Lock guards are dropped here automatically.
    }

    /// Calls both index preparation methods. (async)
    pub async fn prepare_index(&self) {
        self.prepare_bids_index().await;
        self.prepare_asks_index().await;
    }

    // --- Phase 3: Match Orders ---

    pub async fn process_order<T: ResultSender>(
        &self,
        new_order: Order,
        sender: &T,
    ) -> Vec<MatchedRestingOrder> {
        if new_order.order_type == ORDER_TYPE_BUY || new_order.order_type == ORDER_TYPE_SELL {
            return self.match_order(new_order, sender).await;
        }

        let (_, matched_orders) = self.mock_match_order(new_order, sender).await;
        matched_orders
    }

    /// Primary entry point for matching a new incoming order (aggressor). (async)
    pub async fn match_order<T: ResultSender>(
        &self,
        mut new_order: Order,
        sender: &T,
    ) -> Vec<MatchedRestingOrder> {
        let mut matched_orders: Vec<MatchedRestingOrder> = Vec::new();

        // println!(
        //     "get a new order {:?} and bids size {:?} asks size: {:?}",
        //     new_order.clone(),
        //     self.bids.read().await.len(),
        //     self.asks.read().await.len()
        // );

        let match_sell_side = match new_order.order_type {
            ORDER_TYPE_BUY => true,
            ORDER_TYPE_MOCK_BUY => true,
            ORDER_TYPE_SELL => false,
            ORDER_TYPE_MOCK_SELL => false,
            _ => false, // 或处理未知类型
        };

        matched_orders.extend(
            self.match_against_side(
                &mut new_order,
                match_sell_side, // 使用计算出的标志
                sender,
            )
            .await,
        );

        // Handle the residual new order for LIMIT types
        if new_order.quantity > 0 && new_order.price_type == ORDER_PRICE_TYPE_LIMIT {
            // Unfilled limit order is now resting, add it to the book
            self.fuel_order(new_order).await;
        }

        //println!("get a new matched_orders {:?}", matched_orders.clone());
        matched_orders
    }
    fn safe_duration_u32(end_time: u64, submit_time: u64) -> u32 {
        // 计算差值（防止溢出）
        if end_time < submit_time {
            return 0;
        }
        let max_allowed = u32::MAX;
        let diff = end_time - submit_time;
        if diff > max_allowed as u64 {
            0
        } else {
            diff as u32
        }
    }
    /// Internal function to match a new order against one side (Bids or Asks). (async)
    async fn match_against_side<T: ResultSender>(
        &self,
        new_order: &mut Order,
        match_against_asks: bool,
        sender: &T,
    ) -> Vec<MatchedRestingOrder> {
        let mut matched_orders: Vec<MatchedRestingOrder> = Vec::new();
        let start_time = current_timestamp();
        let timer = HighResultionCounter::start(3.0);
        loop {
            // Break condition: new order is fully filled.
            if new_order.quantity == 0 {
                break;
            }

            // Acquire read locks asynchronously
            let top_index_guard = if match_against_asks {
                self.top_asks_index.read().await
            } else {
                self.top_bids_index.read().await
            };

            let resting_orders_guard = if match_against_asks {
                self.asks.read().await
            } else {
                self.bids.read().await
            };

            // Check if there are any indexed orders left
            if top_index_guard.is_empty() {
                // Try to refill the index if it is empty
                drop(top_index_guard); // Release read lock to allow write lock for preparation

                // Re-index:
                if match_against_asks {
                    self.prepare_asks_index().await
                } else {
                    self.prepare_bids_index().await
                }

                // Re-acquire the lock to check if re-indexing succeeded
                let re_indexed_guard = if match_against_asks {
                    self.top_asks_index.read().await
                } else {
                    self.top_bids_index.read().await
                };

                if re_indexed_guard.is_empty() {
                    break; // Still empty, stop matching
                }

                // Continue loop to use the new index
                continue;
            }

            // Get the index of the best resting order (index 0 in the top list)
            let resting_order_index = top_index_guard[0];

            let resting_order = match resting_orders_guard.get(resting_order_index as usize) {
                Some(order) => order,
                None => {
                    break;
                }
            };

            // --- Price Check ---
            let price_check_ok = if match_against_asks {
                // New BUY vs ASK. New order must have price >= resting price (or be Market).
                new_order.price_type == ORDER_PRICE_TYPE_MARKET
                    || new_order.price >= resting_order.price
            } else {
                // New SELL vs BID. New order must have price <= resting price (or be Market).
                new_order.price_type == ORDER_PRICE_TYPE_MARKET
                    || new_order.price <= resting_order.price
            };

            if !price_check_ok {
                break; // Price not aggressive enough. Stop matching.
            }

            // --- Match Calculation ---
            let trade_quantity = new_order.quantity.min(resting_order.quantity);
            let trade_price = resting_order.price; // Trade price is the resting order's price

            // Update the quantity of the aggressor order
            new_order.quantity -= trade_quantity;

            // Record the matched resting order for post_match cleanup
            matched_orders.push(MatchedRestingOrder {
                order_index: resting_order_index,
                matched_quantity: trade_quantity,
                is_buy: !match_against_asks,
            });

            // Send the MatchResult signal
            let (buy_id, sell_id) = if new_order.order_type == ORDER_TYPE_BUY {
                (new_order.order_id, resting_order.order_id)
            } else {
                (resting_order.order_id, new_order.order_id)
            };

            let time_lapsed = timer.ns();
            let end_time = start_time + (time_lapsed as u64);

            let match_result = MatchResult {
                instance_tag: self.instance_tag,
                product_id: new_order.product_id,
                buy_order_id: buy_id,
                sell_order_id: sell_id,
                price: trade_price,
                quantity: trade_quantity,
                trade_time_network: Self::safe_duration_u32(end_time, new_order.submit_time),
                internal_match_time: (time_lapsed) as u32,
            };

            sender.send_result(match_result).await;

            // Remove the index of the matched resting order from the top list
            // NOTE: Must drop read guards before acquiring the write guard for the index list
            drop(top_index_guard);
            drop(resting_orders_guard);

            let mut top_index_write_guard = if match_against_asks {
                self.top_asks_index.write().await
            } else {
                self.top_bids_index.write().await
            };

            // Remove the first index (the index of the matched order)
            if !top_index_write_guard.is_empty() {
                top_index_write_guard.remove(0);
            }
            drop(top_index_write_guard);

            // Loop continues to check if more orders can be matched.
        }
        let result = matched_orders.clone();
        self.post_match(result).await;
        matched_orders
    }

    // --- Phase 4: Post Match Processing ---

    /// Cleans up the order book after a match, deleting/updating resting orders, and rebuilding indices. (async)
    pub async fn post_match(&self, matched_orders: Vec<MatchedRestingOrder>) {
        let mut bids_to_remove: Vec<OrderIndex> = Vec::new();
        let mut asks_to_remove: Vec<OrderIndex> = Vec::new();

        // Acquire write locks for both bids and asks vectors
        let mut bids_guard = self.bids.write().await;
        let mut asks_guard = self.asks.write().await;

        // 1 & 2. Process and mark for removal/update
        for matched in matched_orders {
            let (orders_vec, to_remove_list) = if matched.is_buy {
                (&mut bids_guard, &mut bids_to_remove)
            } else {
                (&mut asks_guard, &mut asks_to_remove)
            };

            if let Some(order) = orders_vec.get_mut(matched.order_index as usize) {
                if matched.matched_quantity >= order.quantity {
                    // Mark for removal
                    to_remove_list.push(matched.order_index);
                } else {
                    // Partial fill: update remaining quantity
                    order.quantity -= matched.matched_quantity;
                }
            }
        }

        // 2. Remove fully matched orders (must be done in descending index order for safe removal)

        // Remove from Bids
        bids_to_remove.sort_by(|a, b| b.cmp(a));
        for index in bids_to_remove {
            if (index as usize) < bids_guard.len() {
                bids_guard.remove(index as usize);
            }
        }

        // Remove from Asks
        asks_to_remove.sort_by(|a, b| b.cmp(a));
        for index in asks_to_remove {
            if (index as usize) < asks_guard.len() {
                asks_guard.remove(index as usize);
            }
        }

        // Release order vector locks before rebuilding indices
        drop(bids_guard);
        drop(asks_guard);

        // 3. Rebuild the top indices
        self.prepare_bids_index().await;
        self.prepare_asks_index().await;
    }

    /// Attempts to cancel an order by its ID.
    /// Returns `true` if the order was found and canceled, `false` otherwise.
    pub async fn cancel_order(&self, cancel_order_ids: Vec<u64>) -> bool {
        // --- 1. Scan Bids and Asks for Order ID to get the array index ---
        // This array index is needed for removal and to check the top index vector.
        let order_id = *cancel_order_ids.get(0).unwrap(); //support one for now
        let mut order_array_index: Option<(OrderIndex, bool)> = None; // (index, is_buy)

        // Acquire read locks on bids and asks
        let bids_guard = self.bids.read().await;
        let asks_guard = self.asks.read().await;

        // Search Bids for the Order ID
        if let Some((index, _)) = bids_guard
            .iter()
            .enumerate()
            .find(|(_, order)| order.order_id == order_id)
        {
            order_array_index = Some((index as OrderIndex, true));
        }

        // Search Asks for the Order ID
        if order_array_index.is_none() {
            if let Some((index, _)) = asks_guard
                .iter()
                .enumerate()
                .find(|(_, order)| order.order_id == order_id)
            {
                order_array_index = Some((index as OrderIndex, false));
            }
        }

        // Drop read locks on bids/asks
        drop(bids_guard);
        drop(asks_guard);

        let (index_to_remove, is_buy) = match order_array_index {
            Some(data) => data,
            None => return false, // Order not found, nothing to cancel
        };

        // --- 2. Scan Top Index and Clear if Order is in the Top ---
        let mut top_index_write_guard = if is_buy {
            self.top_bids_index.write().await
        } else {
            self.top_asks_index.write().await
        };

        // If the order's array index is present in the top index list, clear the list.
        if top_index_write_guard.contains(&index_to_remove) {
            top_index_write_guard.clear();
        }

        // Drop the write lock on the top index
        drop(top_index_write_guard);

        // --- 3. Remove from Bids or Asks Array ---

        // Acquire the write lock on the correct order vector
        if is_buy {
            let mut bids_guard = self.bids.write().await;
            // Remove the order. Note: Vec::remove is O(N) but simplifies the example.
            if (index_to_remove as usize) < bids_guard.len() {
                bids_guard.remove(index_to_remove as usize);
            }
            drop(bids_guard); // Release lock before re-indexing
        } else {
            let mut asks_guard = self.asks.write().await;
            if (index_to_remove as usize) < asks_guard.len() {
                asks_guard.remove(index_to_remove as usize);
            }
            drop(asks_guard); // Release lock before re-indexing
        }

        // --- 4. Rebuild the indices ---
        // Must be done after removal because array indices for other orders may have changed.
        self.prepare_index().await;

        true // Order was successfully canceled
    }

    // support mock
    async fn mock_match_against_side<T: ResultSender>(
        new_order: &mut Order,
        match_against_asks: bool,
        sender: &T,
        // The large order list is passed as an immutable slice/reference (no clone cost)
        resting_orders: &[Order],
        // The index list is passed as a mutable reference to the local clone (allows modification)
        top_index: &mut Vec<OrderIndex>,
        instance_tag: [u8; 8],
    ) -> Vec<MatchedRestingOrder> {
        let mut matched_orders: Vec<MatchedRestingOrder> = Vec::new();
        let start_time = current_timestamp();
        let timer = HighResultionCounter::start(3.0);

        loop {
            // Stop conditions: aggressor filled or no more top resting orders.
            if new_order.quantity == 0 || top_index.is_empty() {
                break;
            }

            let resting_order_index_in_vector = top_index[0];

            // Access the resting order using the immutable reference to the large data set.
            let resting_order = match resting_orders.get(resting_order_index_in_vector as usize) {
                Some(order) => order,
                None => {
                    top_index.remove(0);
                    continue;
                }
            };

            // --- Price Check ---
            let price_check_ok = if match_against_asks {
                // New BUY vs ASK: New price must be >= resting price (or Market)
                new_order.price_type == ORDER_PRICE_TYPE_MARKET
                    || new_order.price >= resting_order.price
            } else {
                // New SELL vs BID: New price must be <= resting price (or Market)
                new_order.price_type == ORDER_PRICE_TYPE_MARKET
                    || new_order.price <= resting_order.price
            };

            if !price_check_ok {
                break; // Price not aggressive enough.
            }

            // --- Match Calculation ---
            let trade_quantity = new_order.quantity.min(resting_order.quantity);
            let trade_price = resting_order.price;

            new_order.quantity -= trade_quantity;

            matched_orders.push(MatchedRestingOrder {
                order_index: resting_order_index_in_vector,
                matched_quantity: trade_quantity,
                is_buy: !match_against_asks,
            });

            // Determine Buy/Sell IDs for the trade result
            let (buy_id, sell_id) = if !match_against_asks {
                // Matching BIDS (BUY side) -> Resting order is BUY
                (resting_order.order_id, new_order.order_id)
            } else {
                // Matching ASKS (SELL side) -> Resting order is SELL
                (new_order.order_id, resting_order.order_id)
            };

            let time_lapsed = timer.ns();
            let end_time = start_time + (time_lapsed as u64);

            let mock_result = MatchResult {
                instance_tag: instance_tag,
                product_id: new_order.product_id,
                buy_order_id: buy_id,
                sell_order_id: sell_id,
                price: trade_price,
                quantity: trade_quantity,
                trade_time_network: Self::safe_duration_u32(end_time, new_order.submit_time),
                internal_match_time: (time_lapsed) as u32,
            };

            // Send the mock trade signal
            sender.send_result(mock_result).await;

            // Consume the top index from the local clone
            top_index.remove(0);
        }
        matched_orders
    }
    // ----------------------------------------------------------------------

    /// Simulates order matching against the current order book state.
    /// It reads from the OrderBook's vectors but modifies local copies of the top indices.
    /// This ensures the OrderBook's state remains unchanged (pure read operation).
    pub async fn mock_match_order<T: ResultSender>(
        &self,
        mut new_order: Order,
        sender: &T,
    ) -> (Order, Vec<MatchedRestingOrder>) {
        let match_against_asks = match new_order.order_type {
            ORDER_TYPE_BUY | ORDER_TYPE_MOCK_BUY => true, // Match against Asks (SELL side)
            ORDER_TYPE_SELL | ORDER_TYPE_MOCK_SELL => false, // Match against Bids (BUY side)
            _ => return (new_order, Vec::new()),
        };

        // --- 1. Acquire Read Locks and Clone Top Index ---

        // Acquire read guards for the side being matched
        let (resting_orders_guard, top_index_guard) = if match_against_asks {
            let asks = self.asks.read().await;
            let top_asks = self.top_asks_index.read().await;
            (asks, top_asks)
        } else {
            let bids = self.bids.read().await;
            let top_bids = self.top_bids_index.read().await;
            (bids, top_bids)
        };

        // Clone the index list to a local mutable variable (cheap, allows mutation)
        let mut top_index_clone = top_index_guard.clone();

        // Explicitly drop the top index read lock as it's no longer needed after cloning,
        // but keep the resting_orders_guard to hold the immutable reference.
        drop(top_index_guard);

        // --- 2. Execute Mock Matching ---

        // Pass the immutable reference of the large order list (&resting_orders_guard)
        let matched_orders = Self::mock_match_against_side(
            &mut new_order,
            match_against_asks,
            sender,
            &resting_orders_guard, // Immutable reference to the data within the read guard (avoids clone)
            &mut top_index_clone,  // Mutable reference to the local clone (allows remove)
            self.instance_tag,
        )
        .await;

        // The resting_orders_guard is automatically dropped here, releasing the read lock.

        // --- 3. Return Mock Results ---
        // New order (with residual quantity) and matched orders list.
        (new_order, matched_orders)
    }
}
