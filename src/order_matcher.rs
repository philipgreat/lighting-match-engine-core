use crate::data_types::{
    EngineState, IncomingMessage, MatchResult, ORDER_PRICE_TYPE_LIMIT, ORDER_PRICE_TYPE_MARKET,
    ORDER_TYPE_BUY, ORDER_TYPE_SELL, Order,
};

use std::cmp::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{Receiver, Sender};
/// Handler responsible for the core order matching logic.
pub struct OrderMatcher {
    receiver: Receiver<IncomingMessage>,
    sender: Sender<MatchResult>, // Sender for matched trades
    state: Arc<EngineState>,
}

impl OrderMatcher {
    /// Creates a new OrderMatcher.
    pub fn new(
        receiver: Receiver<IncomingMessage>,
        sender: Sender<MatchResult>,
        state: Arc<EngineState>,
    ) -> Self {
        OrderMatcher {
            receiver,
            sender,
            state,
        }
    }

    /// Runs the main loop to receive messages and execute matching logic.
    pub async fn run_matching_loop(&mut self) {
        println!("Order matcher started, awaiting messages...");
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                IncomingMessage::Order(order) => self.handle_order_submission(order).await,
                IncomingMessage::Cancel(cancel) => {
                    self.handle_order_cancellation(cancel.order_id).await
                }
            }
        }
    }

    /// Utility function to get the current nanosecond timestamp.
    fn current_timestamp() -> u64 {
        //time::Instant::now().elapsed().as_nanos() as u64
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;
        now_nanos
    }

    fn find_best_match_index(
        book: &mut Vec<Order>,
        new_order: &Order,
        // 移除了 is_buy: bool, is_limit: bool,
    ) -> Option<usize> {
        // ⭐ 优化点 1: 在方法内部推导出布尔值
        let is_buy = new_order.order_type == ORDER_TYPE_BUY;
        let is_limit = new_order.price_type == ORDER_PRICE_TYPE_LIMIT;

        // 1. Filter out all potential counter-orders that satisfy the cross-price condition.
        let potential_matches: Vec<usize> = book
            .iter()
            .enumerate()
            .filter_map(|(i, existing_order)| {
                let is_opposite_side = existing_order.order_type != new_order.order_type;

                if !is_opposite_side {
                    return None; // Must be opposite side (Buy vs. Sell)
                }

                // Check if the price condition is met (i.e., the price crosses)
                let price_condition_met = (is_buy && existing_order.price <= new_order.price) || // New Buy matches existing Sell <= Buyer's Limit Price
                                          (!is_buy && existing_order.price >= new_order.price); // New Sell matches existing Buy >= Seller's Limit Price

                // Market orders match any counter-side order regardless of price
                let market_order_match =
                    new_order.price_type == ORDER_PRICE_TYPE_MARKET && is_opposite_side;

                if (is_limit && price_condition_met) || market_order_match {
                    Some(i)
                } else {
                    None
                }
            })
            .collect(); // Collect indices of all executable orders

        // 2. Apply "Price Priority then Time Priority" using min_by
        potential_matches.into_iter().min_by(|&i_a, &i_b| {
            let order_a = &book[i_a];
            let order_b = &book[i_b];

            // --- Price Priority ---
            let price_cmp = if is_buy {
                // When the new order is a Buy, we look for the cheapest Sell (Lowest Price is Best)
                order_a.price.cmp(&order_b.price)
            } else {
                // When the new order is a Sell, we look for the most expensive Buy (Highest Price is Best)
                // Reverse the comparison (b vs a) to find the maximum price using min_by.
                order_b.price.cmp(&order_a.price)
            };

            if price_cmp != Ordering::Equal {
                return price_cmp;
            }

            // --- Time Priority ---
            // If prices are equal, use Time Priority (Earliest submit_time is Best).
            order_a.submit_time.cmp(&order_b.submit_time)
        })
    }
    /// Handles an incoming order (Limit or Market).
    async fn handle_order_submission(&self, new_order: Order) {
        // Only process orders for the configured product_id
        if new_order.product_id != self.state.product_id {
            eprintln!(
                "Order rejected: Mismatched Product ID (Engine: {}, Order: {})",
                self.state.product_id, new_order.product_id
            );
            return;
        }

        let mut order_book = self.state.order_book.lock().await;
        if order_book.is_empty() {
            println!("no orders in book: {:?}", order_book);
            order_book.push(new_order);
            return;
        }

        // 1. Pre-matching clean-up: Remove expired orders
        self.cleanup_expired_orders(&mut order_book);
        println!(
            "==========> after cleanup_expired_orders orders in book: {:?}",
            order_book.len()
        );

        // 2. Execute matching
        self.match_orders(new_order, &mut order_book).await;
    }

    /// Removes expired orders from the order book.
    fn cleanup_expired_orders(&self, book: &mut Vec<Order>) {
        let now = Self::current_timestamp();
        // Retain only non-expired orders (expire_time == 0 OR expire_time > now)
        book.retain(|order| order.expire_time == 0 || order.expire_time > now);
    }

    /// Handles order cancellation by removing the matching order from the book.
    async fn handle_order_cancellation(&self, order_id_to_cancel: u64) {
        let mut order_book = self.state.order_book.lock().await;

        if let Some(index) = order_book
            .iter()
            .position(|o| o.order_id == order_id_to_cancel)
        {
            order_book.remove(index);
            println!("Order Cancelled: OrderID={}", order_id_to_cancel);
        } else {
            println!(
                "Cancellation failed: OrderID={} not found.",
                order_id_to_cancel
            );
        }
    }

    /// Core matching logic (Price/Time Priority).
    async fn match_orders(&self, mut new_order: Order, book: &mut Vec<Order>) {
        let is_limit = new_order.price_type == ORDER_PRICE_TYPE_LIMIT;
        let is_buy = new_order.order_type == ORDER_TYPE_BUY;

        let mut matches_occurred = true;

        //println!("========>order size for matching {:?}", book.len());

        if book.is_empty() {
            //println!("========> no orders in book: {:?}", book);
            book.push(new_order);
            return;
        }

        let start_time = Self::current_timestamp();

        while new_order.quantity > 0 && matches_occurred {
            matches_occurred = false;
            //println!("1======>check new order {:?} is comming", new_order);
            // 1. Find the best potential match index based on price and time
            let best_match_index = Self::find_best_match_index(book, &new_order);

            //println!("best match index {:?} ", best_match_index);

            if let Some(i) = best_match_index {
                //println!("match!!!!");
                // We found a match!
                matches_occurred = true;
                let mut existing_order = book.remove(i);

                let trade_qty = std::cmp::min(new_order.quantity, existing_order.quantity);
                let trade_price = existing_order.price; // Maker (existing) price is always the trade price

                // 3. Update remaining quantities
                new_order.quantity -= trade_qty;
                existing_order.quantity -= trade_qty;

                // 4. Create MatchResult
                let match_result = MatchResult {
                    instance_tag: self.state.instance_tag,
                    product_id: self.state.product_id,
                    buy_order_id: if is_buy {
                        new_order.order_id
                    } else {
                        existing_order.order_id
                    },
                    sell_order_id: if !is_buy {
                        new_order.order_id
                    } else {
                        existing_order.order_id
                    },
                    price: trade_price,
                    quantity: trade_qty,
                    trade_time_network: (Self::current_timestamp() - new_order.submit_time) as u32,
                    internal_match_time: (Self::current_timestamp() - start_time) as u32,
                };
                //println!("=========>result generated");
                // 5. Broadcast the match result
                if let Err(e) = self.sender.send(match_result).await {
                    eprintln!("Error sending match result: {}", e);
                }

                // 6. Update matched orders counter
                let mut matched_count = self.state.matched_orders.lock().await;
                *matched_count += 1;

                // 7. If the existing order has remaining quantity, push it back
                if existing_order.quantity > 0 {
                    // Note: Order is Copyable, so we clone to push back, retaining the original
                    // instance for further potential match result reporting if needed (though not strictly necessary here, it's safer).
                    // Since Order does not implement Copy, we must use clone()
                    book.push(existing_order.clone());
                }
            }
        }

        // 8. If the new order is a Limit Order and has remaining quantity, add it to the book
        if new_order.quantity > 0 && is_limit {
            book.push(new_order);
            println!(
                "Added partial or full Limit Order to book. Qty Left: {}",
                book.last().unwrap().quantity
            );
        } else if new_order.quantity > 0 && new_order.price_type == ORDER_PRICE_TYPE_MARKET {
            println!(
                "Unfilled Market Order discarded. Qty Left: {}",
                new_order.quantity
            );
            println!("order data {:?}", new_order);
        }
    }
}
