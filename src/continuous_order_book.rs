
use std::thread::sleep;

use crate::date_time_tool::current_timestamp;
use crate::high_resolution_timer::HighResolutionCounter;
// Assuming these are defined in data_types.rs
// NOTE: In a real Rust project, you'd replace 'crate::data_types' with the actual path.
use crate::data_types::{
    OrderExecution, ORDER_PRICE_TYPE_LIMIT, ORDER_PRICE_TYPE_MARKET, ORDER_TYPE_BUY, ORDER_TYPE_SELL,
    Order, ContinuousOrderBook, OrderIndex,MatchResult,MatchedRestingOrder
};

// --- Helper Structs and Trait ---

/// A temporary structure to hold the information of the resting order involved in a match
/// so that it can be processed in post_match (e.g., deletion or quantity update).


/// The core trait for sending match results (trade signals) to an external system.
/// The implementation will be external to this file.
pub trait ResultSender: Send {
    fn send_result(&self, result: MatchResult);
    // fn send_results(&self, results: Vec<OrderExecution>);
    
}

// --- ContinuousOrderBook Definition ---

// pub struct ContinuousOrderBook {
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
impl MatchResult {
     pub fn new(init_trade_size: usize) -> Self{
        MatchResult {
            start_time:0,
            end_time:0,
            order_execution_list: Vec::with_capacity(init_trade_size),
        }
     }
     pub fn add_order_execution(&mut self,trade: OrderExecution){
        self.order_execution_list.push(trade);
     }
     pub fn total_count(& self)->u32{
        self.order_execution_list.len() as u32
     }
     pub fn total_time(& self)-> u64{
       self.end_time - self.start_time
     }
     pub fn time_per_trade(&self)->u32{
        if self.total_count() == 0 {
            return 0
        }
        (self.total_time() / self.total_count() as u64) as u32
     }
     
     
}
impl ContinuousOrderBook {
    /// Constructs a new ContinuousOrderBook with specified initial capacities.
    pub fn new(initial_book_size: u32, initial_top_size: u32) -> Self {
        ContinuousOrderBook {
            bids: Vec::with_capacity(initial_book_size as usize),
            asks: Vec::with_capacity(initial_book_size as usize),

            top_bids_index: Vec::with_capacity(initial_top_size as usize),
            top_asks_index: Vec::with_capacity(initial_top_size as usize),

            init_order_book_size: initial_book_size,
            init_top_index_size: initial_top_size,

            bids_index_used: 0,
            asks_index_used: 0,

            total_ask_volumn: 0,
            total_bid_volumn: 0,

            matched_orders: Vec::with_capacity(initial_top_size as usize),
            bids_to_remove: Vec::with_capacity(initial_top_size as usize ),
            asks_to_remove: Vec::with_capacity(initial_top_size as usize ),
            
            match_result: MatchResult::new(initial_top_size as usize),

        }
    }

    // --- Phase 1: Fuel Order (Adding orders) ---

    /// Adds an order to the order book (bids or asks).
    pub fn fuel_order(&mut self, order: Order) {
        if order.order_type == ORDER_TYPE_BUY {
            self.bids.push(order);
        } else if order.order_type == ORDER_TYPE_SELL {
            self.asks.push(order);
        }
    }

    // --- Phase 2: Index Preparation ---

    /// Finds and stores the indices of the best bid orders. (async)
    // --- Phase 2: Index Preparation ---
    fn need_to_rebuild_bids_index(&self) -> bool {
        (self.bids_index_used == 0 && self.top_bids_index.len() == 0 ) //make simple path faster
        || (self.bids_index_used >= self.top_bids_index.len())
    }
    fn need_to_rebuild_asks_index(&self) -> bool {
        (self.asks_index_used == 0 && self.top_asks_index.len() == 0 ) //make simple path faster
        || (self.asks_index_used >= self.top_asks_index.len())
    }
    /// Finds and stores the indices of the best bid orders based on Price (desc) then Time (asc). (async)
    fn prepare_bids_index(&mut self) {
        //do not need rebuild index when

        // if !self.need_to_rebuild_bids_index() {
        //     println!(
        //         "!self.need_to_rebuild_bids_index() {}-->{}",
        //         self.bids_index_used,
        //         self.top_bids_index.len()
        //     );
        //     return;
        // }

        // 2. Create a list of (index, price, submit_time) for sorting
        let mut indexed_bids: Vec<(OrderIndex, u64, u64)> = self
            .bids
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

        //let mut top_bids_index_guard = self.top_bids_index.write().await;
        self.top_bids_index.clear();
        self.bids_index_used = 0;

        // 5. Take the first N indices (top orders)
        let max_size = self.init_top_index_size as usize;
        for (index, _, _) in indexed_bids.into_iter().take(max_size) {
            self.top_bids_index.push(index);
        }

        // Lock guards are dropped here automatically.
    }

    /// Finds and stores the indices of the best ask orders based on Price (asc) then Time (asc). (async)
    fn prepare_asks_index(&mut self) {
        // 1. Acquire read lock for asks
        // if !self.need_to_rebuild_asks_index() {
        //     println!(
        //         "!self.need_to_rebuild_asks_index() {}-->{}",
        //         self.asks_index_used,
        //         self.top_asks_index.len()
        //     );

        //     return;
        // }

        // 2. Create a list of (index, price, submit_time) for sorting
        let mut indexed_asks: Vec<(OrderIndex, u64, u64)> = self
            .asks
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
        //let mut top_asks_index_guard = self.top_asks_index.write().await;
        self.top_asks_index.clear();
        self.asks_index_used = 0;
        // 5. Take the first N indices (top orders)
        let max_size = self.init_top_index_size as usize;
        for (index, _, _) in indexed_asks.into_iter().take(max_size) {
            self.top_asks_index.push(index);
        }

        // Lock guards are dropped here automatically.
    }

    /// Calls both index preparation methods. (async)
    pub fn prepare_index(&mut self) {
        self.prepare_bids_index();
        self.prepare_asks_index();
    }
    pub fn update_stats(&mut self){

        self.total_ask_volumn = self.asks.iter().map(|item| item.quantity).sum();
        self.total_bid_volumn = self.bids.iter().map(|item| item.quantity).sum();
    }
    // when match ends, call the func to update stats data
    pub fn update_stats_with_result(&mut self, new_order:&Order){

        let is_sell_order = if new_order.order_type == ORDER_TYPE_SELL {
            true
        }else {
            false
        };

        if is_sell_order && new_order.quantity > 0{
            self.total_ask_volumn += new_order.quantity            
        }else {
            self.total_bid_volumn += new_order.quantity
        }

        if self.match_result.order_execution_list.is_empty() {
            return
        }

        let total_quantity:u32 = 
        self.match_result.order_execution_list.iter().map(|item|item.quantity).sum();

        if is_sell_order {
            self.total_bid_volumn -= total_quantity;
        }else {            
            self.total_ask_volumn -= total_quantity;
        }

        

    }


    // --- Phase 3: Match Orders ---

    /// Primary entry point for matching a new incoming order (aggressor). (async)
    pub async fn match_order<T: ResultSender>(
        &mut self,
        mut new_order: Order,
        sender: &T,
    ) -> Vec<MatchedRestingOrder> {
        //println!("entering match_order");
        //let mut matched_orders: Vec<MatchedRestingOrder> = Vec::with_capacity(200);
        let mut match_agaist_asks = false;
        if new_order.order_type == ORDER_TYPE_SELL {
            // New order is a SELL, match against Bids (BUY side)
           self.match_against_side(
                    &mut new_order,
                    false, // match against BUY side
                    sender,
                );
        } else if new_order.order_type == ORDER_TYPE_BUY {
            // New order is a BUY, match against Asks (SELL side)
            match_agaist_asks = true;
            self.match_against_side(
                    &mut new_order,
                    true, // match against SELL side
                    sender,
                );
        }
        //println!("entering match_order order type {:?}", new_order);
        // Handle the residual new order for LIMIT types
        if new_order.quantity > 0 && new_order.price_type == ORDER_PRICE_TYPE_LIMIT {
            // Unfilled limit order is now resting, add it to the book
            self.fuel_order(new_order);
            
            if match_agaist_asks  {
                self.prepare_bids_index();
            } else {
                self.prepare_asks_index();
            }

        }
        

        //println!("get a new matched_orders {:?}", matched_orders.clone());
        self.matched_orders.clone()
    }

    /// Internal function to match a new order against one side (Bids or Asks). (async)
    ///
    fn match_against_side<T: ResultSender>(
        &mut self,
        new_order: &mut Order,
        match_against_asks: bool,
        sender: &T,
    ) {
        //let mut matched_orders: Vec<MatchedRestingOrder> = Vec::with_capacity(200);
        let engine_received_time = current_timestamp();
        let timer = HighResolutionCounter::start(28*100_000_000);
        //let mut match_result = MatchResult::new(200);
        self.match_result.order_execution_list.clear();
        let start_time = timer.ns();
        self.match_result.start_time = start_time as u64;
        
        loop {
            
            //println!("1entering match_against_side");
            
            // println!("info: matched order size {:?}", matched_orders.len());
            // println!("info: matched order  {:?}", new_order);
            // Break condition: new order is fully filled.
            if new_order.quantity == 0 {
                //println!("new_order.quantity == 0");
                break;
            }

            let top_index = if match_against_asks {
                &mut self.top_asks_index
            } else {
                &mut self.top_bids_index
            };
            //println!("2entering match_against_side");

            // Check if there are any indexed orders left
            if top_index.is_empty() {
                println!("top_index_guard.is_empty()");
                // Try to refill the index if it is empty

                // Re-index:
                if match_against_asks {
                    self.prepare_asks_index()
                } else {
                    self.prepare_bids_index()
                }

                // Re-acquire the lock to check if re-indexing succeeded
                let re_indexed = if match_against_asks {
                    &self.top_asks_index
                } else {
                    &self.top_bids_index
                };

                if re_indexed.is_empty() {
                    //println!("re_indexed.is_empty() breaking");
                    break; // Still empty, stop matching
                }

                // Continue loop to use the new index
                continue;
            }
            let resting_orders = if match_against_asks {
                &self.asks
            } else {
                &self.bids
            };

            let top_index_used = if match_against_asks {
                self.asks_index_used
            } else {
                self.bids_index_used
            };



            // Get the index of the best resting order (index 0 in the top list)
            let resting_order_index = top_index[top_index_used];
            // println!(
            //     "resting_order_index = top_index[top_index_used] {:?} top index {:?}",
            //     top_index_used, top_index
            // );
            let resting_order = match resting_orders.get(resting_order_index as usize) {
                Some(order) => order,
                None => {
                    eprintln!(
                        "fatal: no order found for index {} !!!",
                        resting_order_index
                    );
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
                println!("!price_check_ok incoming  order {:?} and resting order {:?}", 
                new_order,resting_order);
                break; // Price not aggressive enough. Stop matching.
            }

            // --- Match Calculation ---
            let trade_quantity = new_order.quantity.min(resting_order.quantity);
            let trade_price = resting_order.price; // OrderExecution price is the resting order's price

            // Update the quantity of the aggressor order
            new_order.quantity -= trade_quantity;

            // Record the matched resting order for post_match cleanup
            self.matched_orders.push(MatchedRestingOrder {
                order_index: resting_order_index,
                matched_quantity: trade_quantity,
                is_buy: !match_against_asks,
            });

            // Send the OrderExecution signal
            let (buy_id, sell_id) = if new_order.order_type == ORDER_TYPE_BUY {
                (new_order.order_id, resting_order.order_id)
            } else {
                (resting_order.order_id, new_order.order_id)
            };

            
            if match_against_asks {
                self.asks_index_used = self.asks_index_used + 1;
            } else {
                self.bids_index_used = self.bids_index_used + 1;
            };



            let order_execution = OrderExecution {
                // ... (fields populated) ...
                instance_tag: [0; 16],
                product_id: new_order.product_id,
                buy_order_id: buy_id,
                sell_order_id: sell_id,
                price: trade_price,
                quantity: trade_quantity,
                trade_time_network: (engine_received_time - new_order.submit_time) as u32,
                internal_match_time: 0,
                is_mocked_result: new_order.is_mocked_order,
            };
            self.match_result.add_order_execution(order_execution);
            //sender.send_result(order_execution);

            let needs_to_rebuild_index = if match_against_asks {
                self.need_to_rebuild_asks_index()
            } else {
                self.need_to_rebuild_bids_index()
            };

            if needs_to_rebuild_index {
                self.post_match(match_against_asks);
                
            }

            // Loop continues to check if more orders can be matched.
        }
        //let result = self.matched_orders.clone();
        
        let end_time = timer.ns();
        self.match_result.end_time = end_time as u64;
        
            
        sender.send_result(self.match_result.clone());
        
        self.post_match(match_against_asks);
        self.update_stats_with_result(&new_order);
        
        

        
    }

    // --- Phase 4: Post Match Processing ---

    /// Cleans up the order book after a match, deleting/updating resting orders, and rebuilding indices. (async)
    pub fn post_match(&mut self,match_against_asks:bool) {
        //println!(" orders matched {}", matched_orders.len());

        if self.matched_orders.is_empty() {
            //println!("no order to execute post match");
            return;
        }


        // Acquire write locks for both bids and asks vectors

        // 1 & 2. Process and mark for removal/update
        for matched in &self.matched_orders {
            if matched.is_buy {
                // For buy orders, use bids and bids_to_remove
                let (orders_vec, to_remove_list) = (&mut self.bids, &mut self.bids_to_remove);

                if let Some(order) = orders_vec.get_mut(matched.order_index as usize) {
                    if matched.matched_quantity >= order.quantity {
                        // Mark for removal
                        to_remove_list.push(matched.order_index);
                    } else {
                        // Partial fill: update remaining quantity
                        order.quantity -= matched.matched_quantity;
                    }
                }
            } else {
                // For sell orders, use asks and asks_to_remove
                let (orders_vec, to_remove_list) = (&mut self.asks, &mut self.asks_to_remove);

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
        }

        // 2. Remove fully matched orders (must be done in descending index order for safe removal)

        // Remove from Bids
        self.bids_to_remove.sort_unstable_by(|a, b| b.cmp(a));
        //self.bids_to_remove.dedup();

        for &index in &self.bids_to_remove {
            if index < self.bids.len() as u32{
                self.bids.swap_remove(index as usize);
            }
        }
        self.bids_to_remove.clear();

        // Remove from Asks
        self.asks_to_remove.sort_unstable_by(|a, b| b.cmp(a));
        //self.asks_to_remove.dedup();

        for &index in self.asks_to_remove.iter() {
            if index < self.asks.len() as u32 {
                self.asks.swap_remove(index as usize);
            }
        }
        self.asks_to_remove.clear();

        // Release order vector locks before rebuilding indices

        // 3. Rebuild the top indices
        if match_against_asks {
            self.prepare_asks_index();
        }else{
            self.prepare_bids_index();
        }
        self.matched_orders.clear();
        
    }

    /// Attempts to cancel an order by its ID.
    /// Returns `true` if the order was found and canceled, `false` otherwise.
    pub async fn cancel_order(&mut self, order_id: u64) -> bool {
        // --- 1. Scan Bids and Asks for Order ID to get the array index ---
        // This array index is needed for removal and to check the top index vector.

        let mut order_array_index: Option<(OrderIndex, bool)> = None; // (index, is_buy)

        // Acquire read locks on bids and asks

        // Search Bids for the Order ID
        if let Some((index, _)) = self
            .bids
            .iter()
            .enumerate()
            .find(|(_, order)| order.order_id == order_id)
        {
            order_array_index = Some((index as OrderIndex, true));
        }

        // Search Asks for the Order ID
        if order_array_index.is_none() {
            if let Some((index, _)) = self
                .asks
                .iter()
                .enumerate()
                .find(|(_, order)| order.order_id == order_id)
            {
                order_array_index = Some((index as OrderIndex, false));
            }
        }

        let (index_to_remove, is_buy) = match order_array_index {
            Some(data) => data,
            None => return false, // Order not found, nothing to cancel
        };

        // --- 2. Scan Top Index and Clear if Order is in the Top ---
        if is_buy {
            self.top_bids_index.clear();
        } else {
            self.top_asks_index.clear();
        };

        // If the order's array index is present in the top index list, clear the list.

        // --- 3. Remove from Bids or Asks Array ---

        // Acquire the write lock on the correct order vector
        if is_buy {
            // Remove the order. Note: Vec::remove is O(N) but simplifies the example.
            if (index_to_remove as usize) < self.bids.len() {
                self.bids.swap_remove(index_to_remove as usize);
            }
            
        }else{
            if (index_to_remove as usize) < self.asks.len() {
                self.asks.swap_remove(index_to_remove as usize);
            }
        }

        // --- 4. Rebuild the indices ---
        // Must be done after removal because array indices for other orders may have changed.
        self.prepare_index();

        true // Order was successfully canceled
    }
}
