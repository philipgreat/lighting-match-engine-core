use crate::data_types::{
    EngineState, IncomingMessage, MatchResult, Order, ORDER_PRICE_TYPE_LIMIT,
    ORDER_PRICE_TYPE_MARKET, ORDER_TYPE_BUY, ORDER_TYPE_SELL,
};
use crate::message_codec;

use tokio::net::UdpSocket;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time;

use std::sync::Arc;

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
        OrderMatcher { receiver, sender, state }
    }

    /// Runs the main loop to receive messages and execute matching logic.
    pub async fn run_matching_loop(&mut self) {
        println!("Order matcher started, awaiting messages...");
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                IncomingMessage::Order(order) => self.handle_order_submission(order).await,
                IncomingMessage::Cancel(cancel) => self.handle_order_cancellation(cancel.order_id).await,
            }
        }
    }

    /// Utility function to get the current nanosecond timestamp.
    fn current_timestamp() -> u64 {
        time::Instant::now().elapsed().as_nanos() as u64
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

        // 1. Pre-matching clean-up: Remove expired orders
        self.cleanup_expired_orders(&mut order_book);

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

        if let Some(index) = order_book.iter().position(|o| o.order_id == order_id_to_cancel) {
            order_book.remove(index);
            println!("Order Cancelled: OrderID={}", order_id_to_cancel);
        } else {
            println!("Cancellation failed: OrderID={} not found.", order_id_to_cancel);
        }
    }

    /// Core matching logic (Price/Time Priority).
    async fn match_orders(&self, mut new_order: Order, book: &mut Vec<Order>) {
        let is_limit = new_order.price_type == ORDER_PRICE_TYPE_LIMIT;
        let is_buy = new_order.order_type == ORDER_TYPE_BUY;

        let mut matches_occurred = true;

        while new_order.quantity > 0 && matches_occurred {
            matches_occurred = false;

            // 1. Find the best potential match index based on price and time
            let best_match_index = book.iter().enumerate().filter_map(|(i, existing_order)| {
                let is_opposite_side = existing_order.order_type != new_order.order_type;
                if !is_opposite_side {
                    return None;
                }

                let price_condition_met = 
                    (is_buy && existing_order.price <= new_order.price) || // Buy matches Sell <= LimitPrice
                    (!is_buy && existing_order.price >= new_order.price) || // Sell matches Buy >= LimitPrice
                    new_order.price_type == ORDER_PRICE_TYPE_MARKET; // Market orders match any price

                let market_order_match = 
                    new_order.price_type == ORDER_PRICE_TYPE_MARKET && is_opposite_side;


                if (is_limit && price_condition_met) || market_order_match {
                    Some(i)
                } else {
                    None
                }
            })
            // 2. Apply Time Priority: Find the one with the earliest submit_time
            .min_by_key(|&i| book[i].submit_time);


            if let Some(i) = best_match_index {
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
                    buy_order_id: if is_buy { new_order.order_id } else { existing_order.order_id },
                    sell_order_id: if !is_buy { new_order.order_id } else { existing_order.order_id },
                    price: trade_price,
                    quantity: trade_qty,
                    trade_time: Self::current_timestamp(),
                };
                
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
            println!("Added partial or full Limit Order to book. Qty Left: {}", book.last().unwrap().quantity);
        } else if new_order.quantity > 0 && new_order.price_type == ORDER_PRICE_TYPE_MARKET {
            println!("Unfilled Market Order discarded. Qty Left: {}", new_order.quantity);
        }
    }
}
