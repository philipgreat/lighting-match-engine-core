use crate::data_types::{
    EngineState, ORDER_PRICE_TYPE_LIMIT, ORDER_TYPE_BUY, ORDER_TYPE_SELL, Order,
};

use crate::message_codec;

use std::sync::Arc;

use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;
use tokio::time::{self, Duration};

/// Handler responsible for receiving incoming network messages (Orders/Cancels).
pub struct TestOrderBookBuilder {
    test_order_book_size: u32,
    state: Arc<EngineState>,
}

impl TestOrderBookBuilder {
    /// Creates a new NetworkHandler.
    pub fn new(test_order_book_size: u32, state: Arc<EngineState>) -> Self {
        TestOrderBookBuilder {
            test_order_book_size,
            state,
        }
    }

    /// Runs the main loop to receive and process UDP messages.
    pub async fn start_run(&mut self) {
        let mut order_book = self.state.order_book.lock().await;
        for i in 0..self.test_order_book_size {
            let order = self.create_buy_order(i);
            order_book.push(order);
        }
        for i in 0..self.test_order_book_size {
            let order = self.create_sell_order(i, self.test_order_book_size);
            order_book.push(order);
        }
    }
    pub fn create_buy_order(&self, index: u32) -> Order {
        //let time_now = time::Instant::now().elapsed().as_nanos() as u64;
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;
        Order {
            product_id: self.state.product_id,
            order_id: (index + 1) as u64,
            order_type: ORDER_TYPE_BUY,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            price: (index + 1) as u64,
            quantity: 1,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
        }
    }

    pub fn create_sell_order(&self, index: u32, size: u32) -> Order {
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fail")
            .as_nanos() as u64;

        Order {
            product_id: self.state.product_id,
            order_id: (size + index + 1) as u64,
            order_type: ORDER_TYPE_SELL,
            price_type: ORDER_PRICE_TYPE_LIMIT,
            price: (size + 1 + index) as u64,
            quantity: 1,
            submit_time: time_now,
            expire_time: time_now + 1000 * 1000 * 1000 * 1000 * 10,
        }
    }
}
