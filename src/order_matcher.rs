use crate::data_types::{
    EngineState, IncomingMessage, Order,MatchResult,
};
use crate::continuous_order_book::ResultSender;

use std::sync::Arc;

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

    /// Handles an incoming order (Limit or Market).
    async fn handle_order_submission(&self, new_order: Order) {
        // Only process orders for the configured product_id
       // println!("get a new order {:?}", new_order);
        if new_order.product_id != self.state.product_id {
            eprintln!(
                "Order rejected: Mismatched Product ID (Engine: {}, Order: {})",
                self.state.product_id, new_order.product_id
            );
            return;
        }

        let mut continuous_order_book = self.state.continuous_order_book.write().await;
        //println!("get a new order after await");
        continuous_order_book.match_order(new_order, self).await;

        //continuous_order_book.match_order(new_order, sender)
        // 1. Pre-matching clean-up: Remove expired orders
        //self.cleanup_expired_orders(new_order.clone(), &mut continuous_order_book);
        // println!(
        //     "==========> --tag in book: {:?}",
        //     continuous_order_book.len()
        // );

        // 2. Execute matching
        //self.match_orders(new_order).await;
    }

    /// Removes expired orders and the order with same id from the order book.

    /// Handles order cancellation by removing the matching order from the book.
    async fn handle_order_cancellation(&self, order_id_to_cancel: u64) {
        let continuous_order_book = self.state.continuous_order_book.clone();

        let mut book = continuous_order_book.write().await;
        book.cancel_order(order_id_to_cancel).await;
    }
}
impl ResultSender for OrderMatcher {
    fn send_result(&self, result: MatchResult) {
        let _ = self.sender.try_send(result);
    }

    
}
