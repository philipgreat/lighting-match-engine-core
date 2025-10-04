use crate::date_time_tool::current_timestamp;
use tokio::sync::RwLock;
// Assuming these are defined in data_types.rs
// NOTE: In a real Rust project, you'd replace 'crate::data_types' with the actual path.
use crate::data_types::{
    MatchResult, ORDER_PRICE_TYPE_LIMIT, ORDER_PRICE_TYPE_MARKET, ORDER_TYPE_BUY, ORDER_TYPE_SELL,
    Order, OrderBook, OrderIndex,
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
    pub fn new(initial_book_size: u32, initial_top_size: u32) -> Self {
        OrderBook {
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

    /// Primary entry point for matching a new incoming order (aggressor). (async)
    pub async fn match_order<T: ResultSender>(
        &self,
        mut new_order: Order,
        sender: &T,
    ) -> Vec<MatchedRestingOrder> {
        let mut matched_orders: Vec<MatchedRestingOrder> = Vec::new();

        let start_time = current_timestamp();

        println!(
            "get a new order {:?} and bids size {:?} asks size: {:?}",
            new_order.clone(),
            self.bids.read().await.len(),
            self.asks.read().await.len()
        );
        if new_order.order_type == ORDER_TYPE_SELL {
            // New order is a SELL, match against Bids (BUY side)
            matched_orders.extend(
                self.match_against_side(
                    &mut new_order,
                    false, // match against BUY side
                    sender,
                    start_time,
                )
                .await,
            );
        } else if new_order.order_type == ORDER_TYPE_BUY {
            // New order is a BUY, match against Asks (SELL side)
            matched_orders.extend(
                self.match_against_side(
                    &mut new_order,
                    true, // match against SELL side
                    sender,
                    start_time,
                )
                .await,
            );
        }

        // Handle the residual new order for LIMIT types
        if new_order.quantity > 0 && new_order.price_type == ORDER_PRICE_TYPE_LIMIT {
            // Unfilled limit order is now resting, add it to the book
            self.fuel_order(new_order).await;
        }

        println!("get a new matched_orders {:?}", matched_orders.clone());
        matched_orders
    }

    /// Internal function to match a new order against one side (Bids or Asks). (async)
    async fn match_against_side<T: ResultSender>(
        &self,
        new_order: &mut Order,
        match_against_asks: bool,
        sender: &T,
        start_time: u64,
    ) -> Vec<MatchedRestingOrder> {
        let mut matched_orders: Vec<MatchedRestingOrder> = Vec::new();

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
            let end_time = current_timestamp();
            let match_result = MatchResult {
                // ... (fields populated) ...
                instance_tag: [0; 8],
                product_id: new_order.product_id,
                buy_order_id: buy_id,
                sell_order_id: sell_id,
                price: trade_price,
                quantity: trade_quantity,
                trade_time_network: (end_time - new_order.submit_time) as u32,
                internal_match_time: (end_time - start_time) as u32,
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
        self.post_match(matched_orders).await;
        result
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
}
