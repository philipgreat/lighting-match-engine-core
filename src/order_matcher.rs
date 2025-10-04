use crate::data_types::{
    EngineState, IncomingMessage, MatchResult, ORDER_PRICE_TYPE_LIMIT, ORDER_PRICE_TYPE_MARKET,
    ORDER_TYPE_BUY, ORDER_TYPE_SELL, Order,
};
use crate::order_book::ResultSender;

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

    /// Handles an incoming order (Limit or Market).
    async fn handle_order_submission(&self, new_order: Order) {
        // Only process orders for the configured product_id
        //println!("get a new order {:?}", new_order);
        if new_order.product_id != self.state.product_id {
            eprintln!(
                "Order rejected: Mismatched Product ID (Engine: {}, Order: {})",
                self.state.product_id, new_order.product_id
            );
            return;
        }

        let order_book = self.state.order_book.clone();
        order_book.match_order(new_order, self).await;

        //order_book.match_order(new_order, sender)
        // 1. Pre-matching clean-up: Remove expired orders
        //self.cleanup_expired_orders(new_order.clone(), &mut order_book);
        // println!(
        //     "==========> --tag in book: {:?}",
        //     order_book.len()
        // );

        // 2. Execute matching
        //self.match_orders(new_order).await;
    }

    /// Removes expired orders and the order with same id from the order book.

    /// Handles order cancellation by removing the matching order from the book.
    async fn handle_order_cancellation(&self, order_id_to_cancel: u64) {
        let mut order_book = self.state.order_book.clone();

        //order_book.cancel_order(order_id_to_cancel);
    }
}
impl ResultSender for OrderMatcher {
    /// Implements the required method to send a MatchResult.
    async fn send_result(&self, result: MatchResult) {
        self.sender.send(result).await.expect("send error");
        // println!("result to send: {:?}", result)
    }
}
