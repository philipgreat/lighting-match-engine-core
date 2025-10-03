use crate::data_types::{Order, OrderBook, OrderIndex, PriceLevel};
use crate::date_time_tool::current_timestamp;
use std::collections::BTreeMap;
use std::collections::VecDeque;
impl OrderBook {
    pub fn new(initial_capacity: usize) -> Self {
        // ... (Constructor remains the same) ...
        OrderBook {
            orders: Vec::with_capacity(initial_capacity),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            id_to_index: BTreeMap::new(),
        }
    }

    /// Public method placed within impl OrderBook.
    /// This method is purely responsible for manipulating the order book state (data/indexes).
    /// It requires &mut self, meaning the caller MUST hold a write lock (RwLock::write).
    pub fn place_order(&mut self, order: Order) -> Result<OrderIndex, String> {
        // --- 1. Data Storage and Index Assignment ---
        let new_index = self.orders.len() as OrderIndex;

        if new_index as usize != self.orders.len() {
            return Err("Order index overflowed u32 capacity.".to_string());
        }

        // O(1) append data and O(log N) update ID lookup.
        self.orders.push(order.clone());
        self.id_to_index.insert(order.order_id, new_index);

        // --- 2. Order Layering: Update BTreeMap Index ---
        let price = order.price;
        let map = if order.order_type == 1 {
            // BUY
            &mut self.bids
        } else if order.order_type == 2 {
            // SELL
            &mut self.asks
        } else {
            // Rollback changes
            self.orders.pop();
            self.id_to_index.remove(&order.order_id);
            return Err("Invalid order type.".to_string());
        };

        // O(log N) lookup/insertion in BTreeMap
        map.entry(price)
            .or_insert_with(|| PriceLevel {
                indexes: VecDeque::new(),
            })
            // O(1) append index to the PriceLevel (Time Priority)
            .indexes
            .push_back(new_index);

        Ok(new_index)
    }

    /// Public method to get the Best Bid. Requires &self (read access).
    pub fn get_best_bid(&self) -> Option<(u64, OrderIndex)> {
        // ... (Logic remains the same) ...
        self.bids
            .iter()
            .last()
            .and_then(|(price, level)| level.indexes.front().map(|&idx| (*price, idx)))
    }

    /// Public method to get the Best Ask. Requires &self (read access).
    pub fn get_best_ask(&self) -> Option<(u64, OrderIndex)> {
        // ... (Logic remains the same) ...
        self.asks
            .iter()
            .next()
            .and_then(|(price, level)| level.indexes.front().map(|&idx| (*price, idx)))
    }
    pub fn push(&mut self, order: Order) -> Result<OrderIndex, String> {
        // --- Core logic is identical to the previous place_order method ---
        let new_index = self.orders.len() as OrderIndex;

        if new_index as usize != self.orders.len() {
            return Err("Order index overflowed u32 capacity.".to_string());
        }

        // O(1) append data and O(log N) update ID lookup.
        self.orders.push(order.clone());
        self.id_to_index.insert(order.order_id, new_index);

        // Order Layering logic
        let price = order.price;
        let map = if order.order_type == 1 {
            // BUY
            &mut self.bids
        } else if order.order_type == 2 {
            // SELL
            &mut self.asks
        } else {
            // Rollback changes
            self.orders.pop();
            self.id_to_index.remove(&order.order_id);
            return Err("Invalid order type.".to_string());
        };

        map.entry(price)
            .or_insert_with(|| PriceLevel {
                indexes: VecDeque::new(),
            })
            .indexes
            .push_back(new_index);

        Ok(new_index)
    }

    fn cleanup_expired_orders(&mut self) {
        let now = current_timestamp();
        // Retain only non-expired orders (expire_time == 0 OR expire_time > now)
        self.orders
            .retain(|order| order.expire_time == 0 || order.expire_time > now);
    }

    /// Implements the Vec 'len' behavior. Returns the total number of orders stored.
    /// Requires &self (Read Lock).
    pub fn len(&self) -> usize {
        // The length is simply the length of the underlying contiguous Vec.
        self.orders.len()
    }

    /// Checks if the OrderBook is empty.
    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
    pub fn remove(&mut self, index: usize) -> Order {
        self.orders.remove(index)
    }
    pub fn last(&self) -> Option<&Order> {
        // Vec::last() 返回 Option<&Order>
        self.orders.last()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, Order> {
        self.orders.iter()
    }
    pub fn get_value(&self, index: usize) -> Option<Order> {
        // 2. 使用 Vec::get() 进行安全索引查找，并克隆结果
        //    使用 .get() 而不是 [] 语法，是为了在索引越界时返回 None，而不是 panic。
        self.orders.get(index as usize).cloned()
    }

    pub fn cancel_order(&mut self, order_id_to_cancel: u64) {
        if let Some(index) = self
            .orders
            .iter()
            .position(|o| o.order_id == order_id_to_cancel)
        {
            self.remove(index);
            println!("Order Cancelled: OrderID={}", order_id_to_cancel);
        } else {
            println!(
                "Cancellation failed: OrderID={} not found.",
                order_id_to_cancel
            );
        }
    }
}
